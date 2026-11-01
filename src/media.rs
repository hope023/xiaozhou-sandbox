//! 视频播放：Windows 自带的 Media Foundation（mfreadwrite.dll）解码 + waveOut 出声。
//!
//! 明文不落盘，也不整份进内存：播放器只要求一个 `Source`，读到哪解密到哪，
//! 内存里始终只有一个几 MB 的滚动窗口，因此文件多大都能播。
//! 解码器和音频输出都是系统组件（mfreadwrite / winmm），不引入 FFmpeg 之类
//! 第三方解码器，也没有 GPL 牵连。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::Win32::Media::Audio::{
    waveOutClose, waveOutGetPosition, waveOutOpen, waveOutPause, waveOutPrepareHeader, waveOutReset,
    waveOutRestart, waveOutUnprepareHeader, waveOutWrite, CALLBACK_NULL, HWAVEOUT, WAVEFORMATEX,
    WAVEHDR, WAVE_MAPPER, WHDR_DONE,
};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::Media::{MMTIME, MMTIME_0, TIME_SAMPLES};
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0};
use windows::core::{implement, HRESULT, PWSTR};
use windows::Win32::Foundation::{E_FAIL, E_NOTIMPL, E_POINTER, S_OK, STG_E_INVALIDFUNCTION};
use windows::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, ISequentialStream_Impl, IStream, IStream_Impl, LOCKTYPE,
    STATFLAG, STATSTG, STGC, STGM_READ, STGTY_STREAM, STREAM_SEEK, STREAM_SEEK_CUR,
    STREAM_SEEK_END, STREAM_SEEK_SET, COINIT_MULTITHREADED,
};
use windows::Win32::System::Variant::VT_I8;

const FIRST_VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const FIRST_AUDIO: u32 = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
const ALL_STREAMS: u32 = MF_SOURCE_READER_ALL_STREAMS.0 as u32;
const MEDIA_SOURCE: u32 = MF_SOURCE_READER_MEDIASOURCE.0 as u32;
const FLAG_EOS: u32 = MF_SOURCE_READERF_ENDOFSTREAM.0 as u32;
const FLAG_TYPE_CHANGED: u32 = MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32;

/// 一帧解码好的画面（RGBA8，行紧凑排列）。
pub struct Frame {
    pub pts_ms: u64,
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

/// 按需提供明文字节的数据源。实现方负责解密，播放器只按偏移索取。
///
/// 有了它，播放 5 GB 的视频和播放 5 MB 的视频占用同样的内存：解码器读到哪，
/// 就解到哪，读过的窗口随即被下一段覆盖。
pub trait Source: Send + Sync + 'static {
    /// 明文总长度（字节）。
    fn size(&self) -> u64;
    /// 从 `offset` 开始最多填满 `buf`，返回实际写入的字节数；已到末尾返回 0。
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize, String>;
}

/// UI 与解码线程共享的状态。
pub struct Shared {
    paused: AtomicBool,
    stop: AtomicBool,
    /// 待处理的跳转位置（毫秒），-1 表示没有请求。
    seek_ms: AtomicI64,
    pos_ms: AtomicU64,
    duration_ms: AtomicU64,
    ended: AtomicBool,
    error: Mutex<Option<String>>,
    latest: Mutex<Option<Arc<Frame>>>,
    size: Mutex<(usize, usize)>,
}

impl Shared {
    fn new() -> Self {
        Self {
            paused: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            seek_ms: AtomicI64::new(-1),
            pos_ms: AtomicU64::new(0),
            duration_ms: AtomicU64::new(0),
            ended: AtomicBool::new(false),
            error: Mutex::new(None),
            latest: Mutex::new(None),
            size: Mutex::new((0, 0)),
        }
    }
}

/// 一个正在播放的视频。Drop 时通知解码线程退出。
pub struct Player {
    sh: Arc<Shared>,
}

impl Player {
    /// 用一个按需解密的数据源开播。返回后线程在后台解码。
    pub fn open(src: Arc<dyn Source>) -> Self {
        let sh = Arc::new(Shared::new());
        let bg = Arc::clone(&sh);
        std::thread::Builder::new()
            .name("video-decode".into())
            .spawn(move || decode_loop(src, bg))
            .ok();
        Self { sh }
    }

    pub fn position_ms(&self) -> u64 {
        self.sh.pos_ms.load(Ordering::Relaxed)
    }

    pub fn duration_ms(&self) -> u64 {
        self.sh.duration_ms.load(Ordering::Relaxed)
    }

    pub fn size(&self) -> (usize, usize) {
        *self.sh.size.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_paused(&self) -> bool {
        self.sh.paused.load(Ordering::Relaxed)
    }

    pub fn is_ended(&self) -> bool {
        self.sh.ended.load(Ordering::Relaxed)
    }

    pub fn error(&self) -> Option<String> {
        self.sh.error.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set_paused(&self, paused: bool) {
        self.sh.paused.store(paused, Ordering::Relaxed);
    }

    pub fn toggle_paused(&self) {
        let p = !self.is_paused();
        self.set_paused(p);
    }

    pub fn seek_ms(&self, ms: u64) {
        self.sh.seek_ms.store(ms as i64, Ordering::Relaxed);
        // 跳转后立刻把位置显示改过去，等解码线程真正追上
        self.sh.pos_ms.store(ms, Ordering::Relaxed);
        self.sh.ended.store(false, Ordering::Relaxed);
        // 丢掉跳转前缓存的帧，否则界面上会先闪一下旧画面
        *self.sh.latest.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// 取当前该显示的一帧（取走后不再重复给，避免每帧重建纹理）。
    pub fn take_frame(&self) -> Option<Arc<Frame>> {
        let mut g = self.sh.latest.lock().unwrap_or_else(|e| e.into_inner());
        g.take()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.sh.stop.store(true, Ordering::Relaxed);
    }
}

fn decode_loop(src: Arc<dyn Source>, sh: Arc<Shared>) {
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    unsafe {
        if MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).is_err() {
            fail(&sh, "无法初始化 Media Foundation");
            let _ = CoUninitialize();
            return;
        }
        run(src, &sh);
        let _ = MFShutdown();
    }
    let _ = unsafe { CoUninitialize() };
}

fn fail(sh: &Shared, msg: &str) {
    *sh.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(msg.to_string());
    sh.ended.store(true, Ordering::Relaxed);
}

enum SampleRead {
    Data(Vec<u8>),
    End,
    Skip,
}

enum VideoRead {
    Frame(u64, Arc<Frame>),
    End,
    Skip,
    Err(String),
}

/// 把 `Source` 包成 COM 的 `IStream`，再交给 `MFCreateMFByteStreamOnStream`。
///
/// 这里只做三件事：报长度、定位、按偏移取字节。Media Foundation 的异步读取、
/// 缓冲都由它自己的适配层负责，我们这边保持同步，简单且不会死锁。
#[implement(IStream)]
struct VaultStream {
    src: Arc<dyn Source>,
    pos: Mutex<u64>,
}

impl ISequentialStream_Impl for VaultStream_Impl {
    fn Read(&self, pv: *mut core::ffi::c_void, cb: u32, pcbread: *mut u32) -> HRESULT {
        if pcbread.is_null() || (pv.is_null() && cb > 0) {
            return E_POINTER;
        }
        let mut pos = self.this.pos.lock().unwrap_or_else(|e| e.into_inner());
        let total = self.this.src.size();
        let want = (cb as u64).min(total.saturating_sub(*pos)) as usize;
        if want == 0 {
            unsafe { *pcbread = 0 };
            return S_OK;
        }
        let buf = unsafe { std::slice::from_raw_parts_mut(pv as *mut u8, want) };
        match self.this.src.read_at(*pos, buf) {
            Ok(n) => {
                *pos += n as u64;
                unsafe { *pcbread = n as u32 };
                if n == 0 {
                    E_FAIL
                } else {
                    S_OK
                }
            }
            Err(msg) => {
                unsafe { *pcbread = 0 };
                // 解密失败（篡改/损坏）就当作读错误，让解码器报出来
                eprintln!("[media] 读取明文失败: {msg}");
                E_FAIL
            }
        }
    }

    fn Write(&self, _pv: *const core::ffi::c_void, _cb: u32, _pcbwritten: *mut u32) -> HRESULT {
        E_NOTIMPL
    }
}

impl IStream_Impl for VaultStream_Impl {
    fn Seek(&self, dlibmove: i64, dworigin: STREAM_SEEK, plibnewposition: *mut u64) -> windows::core::Result<()> {
        let total = self.this.src.size() as i64;
        let mut pos = self.this.pos.lock().unwrap_or_else(|e| e.into_inner());
        let base = if dworigin == STREAM_SEEK_SET {
            0
        } else if dworigin == STREAM_SEEK_CUR {
            *pos as i64
        } else if dworigin == STREAM_SEEK_END {
            total
        } else {
            return Err(windows::Win32::Foundation::E_INVALIDARG.into());
        };
        let next = base.saturating_add(dlibmove);
        if next < 0 {
            return Err(STG_E_INVALIDFUNCTION.into());
        }
        *pos = next as u64;
        if !plibnewposition.is_null() {
            unsafe { *plibnewposition = next as u64 };
        }
        Ok(())
    }

    fn SetSize(&self, _libnewsize: u64) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn CopyTo(
        &self,
        _pstm: windows::core::Ref<IStream>,
        _cb: u64,
        _pcbread: *mut u64,
        _pcbwritten: *mut u64,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn Commit(&self, _grfcommitflags: &STGC) -> windows::core::Result<()> {
        Ok(())
    }

    fn Revert(&self) -> windows::core::Result<()> {
        Ok(())
    }

    fn LockRegion(&self, _liboffset: u64, _cb: u64, _dwlocktype: &LOCKTYPE) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn UnlockRegion(&self, _liboffset: u64, _cb: u64, _dwlocktype: u32) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn Stat(&self, pstatstg: *mut STATSTG, _grfstatflag: &STATFLAG) -> windows::core::Result<()> {
        if pstatstg.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe {
            *pstatstg = STATSTG {
                pwcsName: PWSTR::null(),
                r#type: STGTY_STREAM.0 as u32,
                cbSize: self.this.src.size(),
                grfMode: STGM_READ,
                ..Default::default()
            };
        }
        Ok(())
    }

    fn Clone(&self) -> windows::core::Result<IStream> {
        Err(E_NOTIMPL.into())
    }
}

unsafe fn run(src: Arc<dyn Source>, sh: &Shared) {
    // 按需解密：明文只在这个滚动窗口里出现，不碰磁盘也不整份进内存
    let stream: IStream = VaultStream { src, pos: Mutex::new(0) }.into();
    let Ok(byte_stream) = MFCreateMFByteStreamOnStream(&stream) else {
        fail(sh, "MF 字节流创建失败");
        return;
    };

    // 打开视频处理器：很多解码器只吐 NV12，要靠它转成 RGB32
    let mut attrs: Option<IMFAttributes> = None;
    if MFCreateAttributes(&mut attrs, 2).is_err() {
        fail(sh, "创建读取器属性失败");
        return;
    }
    let Some(attrs) = attrs else {
        fail(sh, "创建读取器属性失败");
        return;
    };
    let _ = attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1);

    let reader = match MFCreateSourceReaderFromByteStream(&byte_stream, &attrs) {
        Ok(r) => r,
        Err(e) => {
            fail(sh, &format!("打开视频读取器失败：{e}"));
            return;
        }
    };

    let _ = reader.SetStreamSelection(ALL_STREAMS, false);
    if reader.SetStreamSelection(FIRST_VIDEO, true).is_err() {
        fail(sh, "这个文件里没有视频轨");
        return;
    }

    let Ok(mt) = MFCreateMediaType() else {
        fail(sh, "创建输出格式失败");
        return;
    };
    let _ = mt.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video);
    let _ = mt.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32);
    let _ = mt.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32);
    let _ = mt.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1);

    // 处理器不认缩放尺寸就退回原尺寸
    let accepted = reader.SetCurrentMediaType(FIRST_VIDEO, None, &mt).is_ok() || {
        let Ok(mt2) = MFCreateMediaType() else {
            fail(sh, "创建输出格式失败");
            return;
        };
        let _ = mt2.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video);
        let _ = mt2.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32);
        let _ = mt2.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32);
        reader.SetCurrentMediaType(FIRST_VIDEO, None, &mt2).is_ok()
    };
    if !accepted {
        let codec = reader
            .GetNativeMediaType(FIRST_VIDEO, 0)
            .ok()
            .and_then(|m| m.GetGUID(&MF_MT_SUBTYPE).ok())
            .map(|g| fourcc(g.data1))
            .unwrap_or_else(|| "未知".into());
        fail(sh, &missing_codec_hint(&codec));
        return;
    }

    let (mut w, mut h) = read_size(&reader).unwrap_or((0, 0));
    if w == 0 || h == 0 {
        fail(sh, "拿不到视频尺寸");
        return;
    }
    *sh.size.lock().unwrap_or_else(|e| e.into_inner()) = (w, h);

    if let Ok(pv) = reader.GetPresentationAttribute(MEDIA_SOURCE, &MF_PD_DURATION) {
        let ticks = pv.Anonymous.Anonymous.Anonymous.uhVal;
        if ticks > 0 {
            sh.duration_ms.store(ticks / 10_000, Ordering::Relaxed);
        }
    }

    let mut audio = setup_audio(&reader);

    let mut stride: i32 = (w * 4) as i32;
    let mut clock_base: u64 = 0;
    let mut wall = Instant::now();
    let mut pending: Option<(u64, Arc<Frame>)> = None;
    let mut video_done = false;
    /// 跳转后丢弃早于这个时间戳的陈旧帧，0 表示不丢。
    let mut skip_before: u64 = 0;
    /// 视频到结尾后开始计时的时刻，用来给音频收尾兜底。
    let mut video_end_at: Option<Instant> = None;

    loop {
        if sh.stop.load(Ordering::Relaxed) {
            break;
        }

        let want = sh.seek_ms.swap(-1, Ordering::Relaxed);
        if want >= 0 {
            let mut raw: PROPVARIANT_0_0 = std::mem::zeroed();
            raw.vt = VT_I8;
            raw.Anonymous.uhVal = (want as u64) * 10_000;
            let pv = PROPVARIANT {
                Anonymous: PROPVARIANT_0 { Anonymous: std::mem::ManuallyDrop::new(raw) },
            };
            if reader
                .SetCurrentPosition(&windows::core::GUID::zeroed(), &pv)
                .is_ok()
            {
                if let Some(a) = audio.as_mut() {
                    a.reset(want as u64);
                }
                clock_base = want as u64;
                wall = Instant::now();
                pending = None;
                video_done = false;
                video_end_at = None;
                // 解码器队列里可能还压着跳转前的帧，追上目标位置之前一律不显示
                skip_before = want as u64;
            }
        }

        let paused = sh.paused.load(Ordering::Relaxed);
        if let Some(a) = audio.as_mut() {
            a.set_paused(paused);
        }
        if paused {
            std::thread::sleep(Duration::from_millis(15));
            continue;
        }

        // ── 音频：先把缓冲喂到水位 ──
        if let Some(a) = audio.as_mut() {
            a.pump();
            let mut guard = 0;
            while a.wants_more() && guard < 8 {
                guard += 1;
                match read_sample(&reader, FIRST_AUDIO) {
                    SampleRead::Data(d) => a.push(&d),
                    SampleRead::End => {
                        a.end();
                        break;
                    }
                    SampleRead::Skip => break,
                }
            }
        }

        // ── 视频：取下一帧 ──
        if pending.is_none() && !video_done {
            match read_video(&reader, &mut w, &mut h, &mut stride, sh) {
                VideoRead::Frame(pts, frame) => pending = Some((pts, frame)),
                VideoRead::End => video_done = true,
                VideoRead::Skip => {}
                VideoRead::Err(msg) => {
                    fail(sh, &msg);
                    break;
                }
            }
        }

        // ── 按播放时钟决定这一帧该不该上屏 ──
        let now_ms = match audio.as_ref() {
            Some(a) if !a.ended() => a.position_ms(),
            _ => clock_base + wall.elapsed().as_millis() as u64,
        };

        match pending.take() {
            Some((pts, frame)) => {
                // 跳转请求还没被这一轮处理掉时，别把旧帧推给界面
                if sh.seek_ms.load(Ordering::Relaxed) >= 0 {
                    continue;
                }
                if skip_before > 0 && pts + 250 < skip_before {
                    continue;
                }
                skip_before = 0;
                if pts <= now_ms + 40 {
                    *sh.latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(frame);
                    sh.pos_ms.store(pts, Ordering::Relaxed);
                } else {
                    pending = Some((pts, frame));
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
            None => {
                if video_done {
                    let (a_ended, queued) = match audio.as_ref() {
                        Some(a) => (a.ended(), a.queued_ms()),
                        None => (true, 0),
                    };
                    let waited = video_end_at.get_or_insert_with(Instant::now).elapsed();
                    // 音频正常放完，或者等太久（设备异常）就收尾
                    let audio_left =
                        !(a_ended && queued == 0) && waited.as_millis() as u64 <= queued + 3000;
                    if !audio_left {
                        // 停在结尾等用户拖进度条 / 重播。
                        // 线程不能退出：一退出就没人再处理 seek，表现就是「播完就死」。
                        sh.ended.store(true, Ordering::Relaxed);
                        sh.pos_ms
                            .store(sh.duration_ms.load(Ordering::Relaxed), Ordering::Relaxed);
                        std::thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }

    if let Some(mut a) = audio.take() {
        a.close();
    }
}

/// 视频格式 GUID 的低 32 位其实是 fourcc，转回 "avc1" / "VP09" 这种可读名字。
fn fourcc(data1: u32) -> String {
    let b = data1.to_le_bytes();
    if b.iter().all(|c| c.is_ascii_graphic()) {
        String::from_utf8_lossy(&b).into_owned()
    } else {
        format!("{data1:08X}")
    }
}

/// 缺解码器时给一句能照着做的提示，而不是甩一个 GUID。
fn missing_codec_hint(codec: &str) -> String {
    let hint = match codec {
        "VP09" | "VP90" => "，可在 Microsoft Store 安装「VP9 视频扩展」",
        "AV01" => "，可在 Microsoft Store 安装「AV1 视频扩展」",
        "HEVC" | "HEV1" | "HVC1" => "，可在 Microsoft Store 安装「HEVC 视频扩展」",
        _ => "，可先用「解密导出」再播放",
    };
    format!("系统缺少 {codec} 解码器{hint}。")
}

/// 一帧最多输出这么宽。4K 源直接上纹理既撞显存上限又白烧带宽，
/// 面板里根本显示不到 1280。
const MAX_OUT_W: usize = 1280;

unsafe fn read_size(reader: &IMFSourceReader) -> Option<(usize, usize)> {
    let mt = reader.GetCurrentMediaType(FIRST_VIDEO).ok()?;
    let packed = mt.GetUINT64(&MF_MT_FRAME_SIZE).ok()?;
    let w = (packed >> 32) as usize;
    let h = (packed & 0xFFFF_FFFF) as usize;
    Some((w, h))
}

unsafe fn read_video(
    reader: &IMFSourceReader,
    w: &mut usize,
    h: &mut usize,
    stride: &mut i32,
    sh: &Shared,
) -> VideoRead {
    let mut actual = 0u32;
    let mut flags = 0u32;
    let mut ts = 0i64;
    let mut sample: Option<IMFSample> = None;
    if reader
        .ReadSample(
            FIRST_VIDEO,
            0,
            Some(&mut actual),
            Some(&mut flags),
            Some(&mut ts),
            Some(&mut sample),
        )
        .is_err()
    {
        return VideoRead::Err("读取视频帧失败".into());
    }
    if flags & FLAG_EOS != 0 {
        return VideoRead::End;
    }
    if flags & FLAG_TYPE_CHANGED != 0 {
        if let Some((nw, nh)) = read_size(reader) {
            if nw > 0 && nh > 0 {
                *w = nw;
                *h = nh;
                *stride = (nw * 4) as i32;
                *sh.size.lock().unwrap_or_else(|e| e.into_inner()) = (nw, nh);
            }
        }
        return VideoRead::Skip;
    }
    let Some(sample) = sample else {
        return VideoRead::Skip;
    };
    let pts_ms = (ts.max(0) as u64) / 10_000;

    let Ok(buf) = sample.ConvertToContiguousBuffer() else {
        return VideoRead::Skip;
    };
    let mut ptr: *mut u8 = std::ptr::null_mut();
    let mut len = 0u32;
    if buf.Lock(&mut ptr, None, Some(&mut len)).is_err() || ptr.is_null() {
        return VideoRead::Skip;
    }
    let rgba = bgra_to_rgba(ptr, len as usize, *w, *h, *stride);
    let _ = buf.Unlock();
    match rgba {
        Some((rgba, ow, oh)) => VideoRead::Frame(pts_ms, Arc::new(Frame { pts_ms, w: ow, h: oh, rgba })),
        None => VideoRead::Skip,
    }
}

unsafe fn read_sample(reader: &IMFSourceReader, stream: u32) -> SampleRead {
    let mut actual = 0u32;
    let mut flags = 0u32;
    let mut ts = 0i64;
    let mut sample: Option<IMFSample> = None;
    if reader
        .ReadSample(stream, 0, Some(&mut actual), Some(&mut flags), Some(&mut ts), Some(&mut sample))
        .is_err()
    {
        return SampleRead::End;
    }
    if flags & FLAG_EOS != 0 {
        return SampleRead::End;
    }
    if flags & FLAG_TYPE_CHANGED != 0 {
        return SampleRead::Skip;
    }
    let Some(sample) = sample else {
        return SampleRead::Skip;
    };
    let Ok(buf) = sample.ConvertToContiguousBuffer() else {
        return SampleRead::Skip;
    };
    let mut ptr: *mut u8 = std::ptr::null_mut();
    let mut len = 0u32;
    if buf.Lock(&mut ptr, None, Some(&mut len)).is_err() || ptr.is_null() || len == 0 {
        let _ = buf.Unlock();
        return SampleRead::Skip;
    }
    let data = std::slice::from_raw_parts(ptr, len as usize).to_vec();
    let _ = buf.Unlock();
    SampleRead::Data(data)
}

/// MF 的 RGB32 在内存里是 BGRA（小端 0x00RRGGBB），换成 RGBA 交给 egui。
/// stride 为负表示自下而上排列。超宽的源在这里整数倍抽样降采样。
unsafe fn bgra_to_rgba(
    ptr: *const u8,
    len: usize,
    w: usize,
    h: usize,
    stride: i32,
) -> Option<(Vec<u8>, usize, usize)> {
    let abs = stride.unsigned_abs() as usize;
    if abs < w * 4 || len < abs * h || w == 0 || h == 0 {
        return None;
    }
    let src = std::slice::from_raw_parts(ptr, len);
    let step = w.div_ceil(MAX_OUT_W).max(1);
    let ow = w / step;
    let oh = h / step;
    if ow == 0 || oh == 0 {
        return None;
    }
    let mut out = vec![0u8; ow * oh * 4];
    for y in 0..oh {
        let sy = y * step;
        let row = if stride >= 0 { sy } else { h - 1 - sy };
        let base = row * abs;
        let d = &mut out[y * ow * 4..(y + 1) * ow * 4];
        for x in 0..ow {
            let i = base + x * step * 4;
            let j = x * 4;
            d[j] = src[i + 2];
            d[j + 1] = src[i + 1];
            d[j + 2] = src[i];
            d[j + 3] = 255;
        }
    }
    Some((out, ow, oh))
}

// ─────────────────────────── 音频输出（waveOut） ───────────────────────────

const BUF_MS: u32 = 80;
const BUF_COUNT: usize = 4;

/// 极简 PCM 播放：几个固定缓冲轮转写进系统混音器。
/// 只用系统 winmm，不引第三方音频库。
struct AudioOut {
    hwo: HWAVEOUT,
    hdrs: Vec<WAVEHDR>,
    bufs: Vec<Vec<u8>>,
    in_use: Vec<bool>,
    pending: VecDeque<u8>,
    buf_size: usize,
    rate: u32,
    block: u16,
    base_ms: u64,
    paused: bool,
    ended: bool,
}

impl AudioOut {
    unsafe fn open(rate: u32, channels: u16, bits: u16) -> Option<Self> {
        let block = channels.checked_mul(bits / 8)?;
        if rate == 0 || block == 0 {
            return None;
        }
        let wfx = WAVEFORMATEX {
            wFormatTag: 1, // WAVE_FORMAT_PCM
            nChannels: channels,
            nSamplesPerSec: rate,
            nAvgBytesPerSec: rate * block as u32,
            nBlockAlign: block,
            wBitsPerSample: bits,
            cbSize: 0,
        };
        let mut hwo = HWAVEOUT(std::ptr::null_mut());
        if waveOutOpen(Some(&mut hwo), WAVE_MAPPER, &wfx, None, None, CALLBACK_NULL) != 0 {
            return None;
        }

        let raw = (rate as usize * block as usize * BUF_MS as usize) / 1000;
        let buf_size = (raw - raw % block as usize).max(block as usize * 256);

        let mut out = AudioOut {
            hwo,
            hdrs: Vec::with_capacity(BUF_COUNT),
            bufs: Vec::with_capacity(BUF_COUNT),
            in_use: Vec::with_capacity(BUF_COUNT),
            pending: VecDeque::new(),
            buf_size,
            rate,
            block,
            base_ms: 0,
            paused: false,
            ended: false,
        };

        for _ in 0..BUF_COUNT {
            let mut buf = vec![0u8; buf_size];
            let mut hdr = WAVEHDR {
                lpData: windows::core::PSTR(buf.as_mut_ptr()),
                dwBufferLength: 0,
                dwBytesRecorded: 0,
                dwUser: 0,
                dwFlags: 0,
                dwLoops: 0,
                lpNext: std::ptr::null_mut(),
                reserved: 0,
            };
            if waveOutPrepareHeader(hwo, &mut hdr, std::mem::size_of::<WAVEHDR>() as u32) != 0 {
                out.close();
                return None;
            }
            out.bufs.push(buf);
            out.hdrs.push(hdr);
            out.in_use.push(false);
        }
        Some(out)
    }

    fn push(&mut self, data: &[u8]) {
        self.pending.extend(data.iter().copied());
    }

    fn wants_more(&self) -> bool {
        self.pending.len() < self.buf_size * 2
    }

    /// 把待播数据填进空闲缓冲。
    fn pump(&mut self) {
        for i in 0..self.hdrs.len() {
            if self.in_use[i] && (self.hdrs[i].dwFlags & WHDR_DONE) != 0 {
                self.in_use[i] = false;
            }
        }
        while self.pending.len() >= self.buf_size {
            let Some(i) = (0..self.hdrs.len()).find(|&i| !self.in_use[i]) else {
                break;
            };
            for k in 0..self.buf_size {
                self.bufs[i][k] = self.pending.pop_front().unwrap_or(0);
            }
            self.hdrs[i].dwBufferLength = self.buf_size as u32;
            let rc = unsafe {
                waveOutWrite(
                    self.hwo,
                    &mut self.hdrs[i],
                    std::mem::size_of::<WAVEHDR>() as u32,
                )
            };
            // 写失败就别占着这个缓冲，否则 WHDR_DONE 永远不会置位，
            // 「等音频播完」就会一直等下去。
            self.in_use[i] = rc == 0;
        }
    }

    /// 已播到哪儿（含跳转起点）。
    fn position_ms(&self) -> u64 {
        let mut t = MMTIME { wType: TIME_SAMPLES, u: MMTIME_0 { sample: 0 } };
        let rc =
            unsafe { waveOutGetPosition(self.hwo, &mut t, std::mem::size_of::<MMTIME>() as u32) };
        if rc != 0 {
            return self.base_ms;
        }
        let samples = unsafe { t.u.sample } as u64;
        self.base_ms + samples * 1000 / self.rate.max(1) as u64
    }

    /// 还有多少音频没播完。
    fn queued_ms(&self) -> u64 {
        let inflight = self.in_use.iter().filter(|x| **x).count() as u64;
        let bytes_per_ms = (self.rate as u64 * self.block as u64).max(1) / 1000;
        inflight * BUF_MS as u64 + self.pending.len() as u64 / bytes_per_ms.max(1)
    }

    fn ended(&self) -> bool {
        self.ended
    }

    fn end(&mut self) {
        self.ended = true;
    }

    fn set_paused(&mut self, paused: bool) {
        if paused == self.paused {
            return;
        }
        self.paused = paused;
        unsafe {
            if paused {
                waveOutPause(self.hwo);
            } else {
                waveOutRestart(self.hwo);
            }
        }
    }

    fn reset(&mut self, to_ms: u64) {
        unsafe {
            waveOutReset(self.hwo);
        }
        for i in 0..self.in_use.len() {
            self.in_use[i] = false;
        }
        self.pending.clear();
        self.base_ms = to_ms;
        self.ended = false;
        self.paused = false;
    }

    fn close(&mut self) {
        if self.hwo.0.is_null() {
            return;
        }
        unsafe {
            waveOutReset(self.hwo);
            for hdr in self.hdrs.iter_mut() {
                waveOutUnprepareHeader(self.hwo, hdr, std::mem::size_of::<WAVEHDR>() as u32);
            }
            waveOutClose(self.hwo);
        }
        self.hwo = HWAVEOUT(std::ptr::null_mut());
    }
}

impl Drop for AudioOut {
    fn drop(&mut self) {
        self.close();
    }
}

unsafe fn setup_audio(reader: &IMFSourceReader) -> Option<AudioOut> {
    if reader.SetStreamSelection(FIRST_AUDIO, true).is_err() {
        return None;
    }
    let mt = MFCreateMediaType().ok()?;
    mt.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).ok()?;
    mt.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).ok()?;
    if reader.SetCurrentMediaType(FIRST_AUDIO, None, &mt).is_err() {
        let _ = reader.SetStreamSelection(FIRST_AUDIO, false);
        return None;
    }
    let cur = reader.GetCurrentMediaType(FIRST_AUDIO).ok()?;
    let rate = cur.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).ok()?;
    let ch = cur.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).ok()? as u16;
    let bits = cur.GetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE).ok()? as u16;
    if rate == 0 || ch == 0 || bits == 0 || bits % 8 != 0 {
        return None;
    }
    AudioOut::open(rate, ch, bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用的数据源：直接读磁盘文件，走的是和保险库一样的按需读取路径。
    struct FileSource {
        f: Mutex<std::fs::File>,
        size: u64,
    }

    impl Source for FileSource {
        fn size(&self) -> u64 {
            self.size
        }

        fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize, String> {
            use std::io::{Read, Seek, SeekFrom};
            let mut f = self.f.lock().unwrap_or_else(|e| e.into_inner());
            f.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
            let mut done = 0;
            while done < buf.len() {
                match f.read(&mut buf[done..]) {
                    Ok(0) => break,
                    Ok(n) => done += n,
                    Err(e) => return Err(e.to_string()),
                }
            }
            Ok(done)
        }
    }

    fn file_source(path: &str) -> Arc<dyn Source> {
        let f = std::fs::File::open(path).unwrap();
        let size = f.metadata().unwrap().len();
        Arc::new(FileSource { f: Mutex::new(f), size })
    }

    /// 用真实文件验证解码链路。没有样本时直接跳过：
    ///   XZ_TEST_VIDEO=D:/some/clip.mp4 cargo test decodes_real
    #[test]
    fn decodes_real_video_when_sample_given() {
        let Ok(path) = std::env::var("XZ_TEST_VIDEO") else {
            return;
        };
        if std::fs::File::open(&path).is_err() {
            return;
        }
        let player = Player::open(file_source(&path));
        for _ in 0..200 {
            if let Some(f) = player.take_frame() {
                assert!(f.w > 0 && f.h > 0, "尺寸异常");
                assert_eq!(f.rgba.len(), f.w * f.h * 4, "RGBA 长度不对");
                assert!(player.duration_ms() > 0, "时长没读到");
                return;
            }
            if let Some(e) = player.error() {
                panic!("解码失败：{e}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("等不到首帧");
    }

    /// 拖动进度条、播完再重播。没有样本时跳过：
    ///   XZ_TEST_VIDEO=D:/some/clip.mp4 cargo test seek_and_replay
    #[test]
    fn seek_and_replay_after_end() {
        let Ok(path) = std::env::var("XZ_TEST_VIDEO") else {
            return;
        };
        if std::fs::File::open(&path).is_err() {
            return;
        }
        let p = Player::open(file_source(&path));

        let mut first = false;
        for _ in 0..200 {
            if p.take_frame().is_some() {
                first = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(first, "首帧没出来");
        let dur = p.duration_ms();
        assert!(dur > 1000, "时长异常：{dur}");

        // 跳到 60%
        let target = dur * 6 / 10;
        p.seek_ms(target);
        let mut landed = None;
        for _ in 0..100 {
            if let Some(f) = p.take_frame() {
                landed = Some(f.pts_ms);
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let pts = landed.expect("跳转后没有新帧");
        assert!(pts + 800 >= target, "跳转没到位：拿到 {pts}，目标 {target}");

        // 跳到接近结尾，等它播完
        p.seek_ms(dur.saturating_sub(400));
        for _ in 0..200 {
            if p.is_ended() {
                break;
            }
            let _ = p.take_frame();
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(p.is_ended(), "没有进入结束状态");

        // 播完之后还能重播（解码线程不能已经退出）
        p.seek_ms(0);
        p.set_paused(false);
        let mut replay = None;
        for _ in 0..100 {
            if let Some(f) = p.take_frame() {
                replay = Some(f.pts_ms);
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let back = replay.expect("重播拿不到帧");
        assert!(back < 1000, "重播没回到开头：{back}");
    }
}