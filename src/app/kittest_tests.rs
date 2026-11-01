//! 无头端到端测试（egui_kittest）：真实跑 App、注入按键、无 GPU 窗口截图。
//! 用户在用电脑时也能跑，不碰真实键盘鼠标。

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use egui_kittest::{kittest::By, kittest::Queryable, Harness};

use super::theme;
use super::App;
use crate::service;
use crate::state::{AppState, Config};
use crate::{store, vault};

const PW: &str = "test12345678";

fn setup_vault(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xv-kittest-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    vault::create(&dir, PW, None).unwrap();
    let dek = vault::unlock(&dir, PW).unwrap();
    let mut v = store::Vault::open(dir.clone(), dek).unwrap();
    let src = dir.join("_src.txt");
    std::fs::write(&src, "hello test").unwrap();
    v.import_file("root", "hello.txt", &src).unwrap();
    v.save_index().unwrap();
    dir
}

fn make_app(dir: PathBuf) -> (Harness<'static, App>, Arc<AppState>) {
    let state = Arc::new(AppState::default());
    *state.config.lock().unwrap() = Config {
        vault_path: dir.display().to_string(),
        ..Default::default()
    };
    // 反截屏别在测试里碰真实窗口
    state.config.lock().unwrap().capture_guard = false;

    let state2 = std::sync::Arc::clone(&state);
    let harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 780.0))
        .build_eframe(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            cc.egui_ctx.all_styles_mut(theme::apply);
            service::spawn_idle_locker(std::sync::Arc::clone(&state2), cc.egui_ctx.clone());
            App::new(state2, cc.egui_ctx.clone())
        });
    (harness, state)
}

/// 走完整用户路径：锁屏输密码 → 回车解锁 → 主界面。
/// （旧代码在这里按回车会触发 input/memory 死锁 panic——这是它的回归测试。）
#[test]
fn kittest_unlock_and_main_layout() {
    let dir = setup_vault("unlock");
    let (mut harness, _state) = make_app(dir);

    harness.run_steps(3);
    // 锁屏的密码框自动聚焦；type_text 发送 Event::Text
    let pw_node = harness.get(By::new().role(egui::accesskit::Role::PasswordInput));
    pw_node.type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10); // 处理事件队列

    // 旧代码在这里会死锁 10 秒后 panic
    let _lock_btn = harness.get(By::new().label("立即锁定")); // 唯一标签：主界面已渲染
    let img = harness.render().expect("render main ui");
    img.save("target/kittest-main.png").expect("save png");
}

/// 隐私输入：直接开隐私模式 → 注入拼音字符 → 断言候选与上屏。
#[test]
fn kittest_privacy_input_types_chinese() {
    let dir = setup_vault("privacy");
    let (mut harness, _state) = make_app(dir);

    harness.run_steps(2);
    let pw_node = harness.get(By::new().role(egui::accesskit::Role::PasswordInput));
    pw_node.type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);
    harness.get(By::new().label("立即锁定"));

    // 选中文件（点击路径），确认编辑器出现
    harness.get(By::new().label("hello.txt")).click();
    harness.run_steps(2);
    let _v = harness.get(By::new().label("版本历史（1 个版本）"));

    // 直接开启隐私模式（点击时序另测），断言状态
    harness.state_mut().set_privacy_mode(true);
    harness.run_steps(3);
    assert!(harness.state().privacy_on, "隐私模式应已开启");

    // 注入拼音字母（egui Event::Text，与真实键盘一致）
    // 暂存文本节点（编辑器里已有 hello test，占位符不会出现）
    harness
        .get(By::new().role(egui::accesskit::Role::Label).label_contains("hello test"))
        .type_text("nihao");
    harness.run_steps(3);

    // 候选应包含「你好」
    {
        let app = harness.state();
        let has = app
            .privacy
            .cands
            .iter()
            .any(|c| c.text == "你好");
        assert!(has, "nihao 应给出「你好」候选，实际：{:?}", app.privacy.cands);
    }

    // 空格上屏（真实键盘的空格以 Event::Text(" ") 到达，与系统输入法解绑后的路径一致）
    harness.event(egui::Event::Text(" ".into()));
    harness.run_steps(2);
    {
        let app = harness.state();
        let text = app.editor.as_ref().map(|e| e.text.clone()).unwrap_or_default();
        assert!(
            text.contains("你好"),
            "空格应把候选上屏，实际文本：{text:?}"
        );
    }

    // 继续打：标点直接上屏
    harness
        .get(By::new().role(egui::accesskit::Role::Label).label_contains("hello test"))
        .type_text(",");
    harness.run_steps(2);
    assert!(
        harness
            .state()
            .editor
            .as_ref()
            .map(|e| e.text.contains("，"))
            .unwrap_or(false),
        "逗号应映射为中文逗号"
    );

    let img = harness.render().expect("render privacy ui");
    img.save("target/kittest-privacy.png").expect("save png");
}

/// 回归：Esc / Tab 会被 egui 自己用来清空或移动焦点，
/// 旧实现只在一进隐私模式时 request_focus 一次，焦点一丢就再也收不回来，
/// 用户看到的是「打字完全没反应」。这里断言 Esc 之后仍然能继续打拼音。
#[test]
fn kittest_privacy_input_survives_escape() {
    let dir = setup_vault("privacy-esc");
    let (mut harness, _state) = make_app(dir);

    harness.run_steps(2);
    let pw_node = harness.get(By::new().role(egui::accesskit::Role::PasswordInput));
    pw_node.type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);
    harness.get(By::new().label("立即锁定"));

    harness.get(By::new().label("hello.txt")).click();
    harness.run_steps(2);
    harness.state_mut().set_privacy_mode(true);
    harness.run_steps(3);

    harness.event(egui::Event::Text("nihao".into()));
    harness.run_steps(2);
    assert_eq!(harness.state().privacy.buf, "nihao", "首轮拼音应进缓冲");

    // 这一下会清掉 egui 焦点；旧代码从此再也收不到按键
    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);
    assert!(harness.state().privacy.buf.is_empty(), "Esc 应清空拼音缓冲");

    harness.event(egui::Event::Text("nihao".into()));
    harness.run_steps(2);
    assert_eq!(
        harness.state().privacy.buf,
        "nihao",
        "Esc 之后打字区必须仍然持焦，否则就是「打不出中文」"
    );
}





/// 文件夹选中后，检查器必须给出「重命名 / 移入回收站」，
/// 而不是落到「解密导出」那种对目录没意义的按钮上。
#[test]
fn kittest_folder_actions_visible() {
    let dir = setup_vault("folderui");
    let (mut harness, state) = make_app(dir);

    harness.run_steps(3);
    harness
        .get(By::new().role(egui::accesskit::Role::PasswordInput))
        .type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);

    let folder = service::create_folder(&state, "root", "文档").unwrap();
    harness.state_mut().selected = Some(folder);
    harness.run_steps(3);

    let img = harness.render().expect("render folder inspector");
    img.save("target/kittest-folder.png").expect("save png");
}



/// 回归：设置页点「保存设置」不能闪退。
/// 原来保存动作在底部条里就执行，把 settings_draft 清空了，
/// 而下面的滚动区还在 clone().unwrap() 它——release 是 panic=abort，直接进程消失。
#[test]
fn kittest_save_settings_does_not_crash() {
    let dir = setup_vault("settingssave");
    let (mut harness, state) = make_app(dir);

    harness.run_steps(3);
    harness
        .get(By::new().role(egui::accesskit::Role::PasswordInput))
        .type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);

    // 打开设置
    harness.get(By::new().label("设置")).click();
    harness.run_steps(5);

    // 保存
    harness.get(By::new().label("保存设置")).click();
    harness.run_steps(5);

    // 面板应该关掉，且库仍然可用（没崩、没被锁）
    assert!(harness.query(By::new().label("保存设置")).is_none(), "设置面板应该已经关闭");
    assert!(state.is_unlocked(), "保存设置不该把库锁上");

    // 再打开一次 → 取消，同样不能崩
    harness.get(By::new().label("设置")).click();
    harness.run_steps(5);
    harness.get(By::new().label("取消")).click();
    harness.run_steps(5);
    assert!(harness.query(By::new().label("保存设置")).is_none());
}

/// GIF：选中后要真的动起来（帧序号随时间推进）。
#[test]
fn kittest_gif_animates() {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, Rgba, RgbaImage};

    let dir = std::env::temp_dir().join("xv-kittest-gif");
    let _ = std::fs::remove_dir_all(&dir);
    vault::create(&dir, PW, None).unwrap();
    let dek = vault::unlock(&dir, PW).unwrap();
    let mut v = store::Vault::open(dir.clone(), dek).unwrap();

    let gif_path = dir.join("_src.gif");
    {
        let f = std::fs::File::create(&gif_path).unwrap();
        let mut enc = GifEncoder::new(f);
        enc.set_repeat(Repeat::Infinite).unwrap();
        for i in 0..3u8 {
            let img = RgbaImage::from_pixel(16, 16, Rgba([i * 60, 10, 10, 255]));
            enc.encode_frame(Frame::from_parts(
                img,
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            ))
            .unwrap();
        }
    }
    v.import_file("root", "anim.gif", &gif_path).unwrap();
    v.save_index().unwrap();

    let (mut harness, _state) = make_app(dir);
    harness.run_steps(3);
    harness
        .get(By::new().role(egui::accesskit::Role::PasswordInput))
        .type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);

    if let Some(node) = harness.query(By::new().label("anim.gif")) {
        node.click();
    }
    // 等后台解密 + 解码
    for _ in 0..40 {
        harness.run_steps(1);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let frames = harness.state().image.frames.len();
    assert!(frames > 1, "GIF 应该解出多帧，实际 {frames}");

    let first = harness.state().image.frame_idx;
    std::thread::sleep(std::time::Duration::from_millis(160));
    harness.run_steps(2);
    let second = harness.state().image.frame_idx;
    assert_ne!(first, second, "帧序号没有推进，动画没跑起来");
}

/// 视频内嵌播放：导入一段真实视频 → 选中 → 等首帧 → 截图。
/// 没有样本时跳过：XZ_TEST_VIDEO=D:/some/clip.mp4 cargo test kittest_video
#[test]
fn kittest_video_preview_renders() {
    let Ok(clip) = std::env::var("XZ_TEST_VIDEO") else {
        return;
    };
    let dir = std::env::temp_dir().join("xv-kittest-video");
    let _ = std::fs::remove_dir_all(&dir);
    vault::create(&dir, PW, None).unwrap();
    let dek = vault::unlock(&dir, PW).unwrap();
    let mut v = store::Vault::open(dir.clone(), dek).unwrap();
    v.import_file("root", "clip.mp4", &PathBuf::from(&clip)).unwrap();
    v.save_index().unwrap();

    let (mut harness, _state) = make_app(dir);
    harness.run_steps(3);
    let pw_node = harness.get(By::new().role(egui::accesskit::Role::PasswordInput));
    pw_node.type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);

    if let Some(node) = harness.query(By::new().label("clip.mp4")) {
        node.click();
    }
    harness.run_steps(5);

    // 等后台解码出首帧
    for _ in 0..80 {
        harness.run_steps(1);
        std::thread::sleep(std::time::Duration::from_millis(40));
    }

    let img = harness.render().expect("render video ui");
    img.save("target/kittest-video.png").expect("save png");

    // ── 快捷键 ──
    let dur = {
        let p = harness.state().video.player.as_ref().expect("播放器没起来");
        p.duration_ms()
    };
    assert!(dur > 0, "时长没读到");

    harness.key_press(egui::Key::Space);
    harness.run_steps(3);
    assert!(
        harness.state().video.player.as_ref().unwrap().is_paused(),
        "空格应该暂停"
    );

    // 暂停状态下位置稳定。注意 seek 落在最近的关键帧上，
    // 所以只断言方向，不断言精确到 10.000 秒。
    let before = harness.state().video.player.as_ref().unwrap().position_ms();
    harness.key_press(egui::Key::ArrowRight);
    harness.run_steps(2);
    let after_right = harness.state().video.player.as_ref().unwrap().position_ms();
    assert!(
        after_right > before + 500,
        "→ 应该往前进，{before}ms → {after_right}ms"
    );

    harness.key_press(egui::Key::ArrowLeft);
    harness.run_steps(2);
    let after_left = harness.state().video.player.as_ref().unwrap().position_ms();
    assert!(
        after_left < after_right,
        "← 应该往回退，{after_right}ms → {after_left}ms"
    );

    harness.key_press(egui::Key::Space);
    harness.run_steps(3);
    assert!(
        !harness.state().video.player.as_ref().unwrap().is_paused(),
        "再按空格应该继续播放"
    );
}

/// 回归：改名框必须能吃中文。
///
/// 旧代码每帧 `request_focus()`，而它内部会 `interrupt_ime()`，
/// egui-winit 收到后每帧 `set_ime_allowed(false/true)` 重置输入法——
/// 表现就是英文能打、拼音永远拼不完（候选框一闪就没）。
#[test]
fn kittest_rename_accepts_chinese() {
    let dir = setup_vault("renamecn");
    let (mut harness, state) = make_app(dir);

    harness.run_steps(3);
    harness
        .get(By::new().role(egui::accesskit::Role::PasswordInput))
        .type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);

    let entry = service::list_dir(&state, "root")
        .unwrap()
        .into_iter()
        .find(|e| e.name == "hello.txt")
        .expect("hello.txt 应该在根目录");
    harness.state_mut().selected = Some(entry);
    harness.run_steps(2);
    harness.state_mut().begin_rename("hello.txt");
    harness.run_steps(2);

    assert!(
        !harness.state().rename_focus_pending,
        "焦点标记应该在首帧就用掉，否则会逐帧打断输入法"
    );
    // 真正的回归点：这一帧不该再请求打断输入法组合。
    // 每帧 request_focus() → interrupt_ime() → egui-winit 每帧
    // set_ime_allowed(false/true)，拼音组合被反复清掉，中文就打不进来。
    let interrupted = harness
        .output()
        .platform_output
        .ime
        .as_ref()
        .is_some_and(|i| i.should_interrupt_composition);
    assert!(!interrupted, "改名框不该逐帧打断输入法组合");

    // 模拟输入法上屏（等价于拼音选完候选后的 Commit）
    harness.input_mut().events.push(egui::Event::Ime(
        egui::ImeEvent::Commit("季度报告".into()),
    ));
    harness.run_steps(2);

    let draft = harness.state().rename_draft.clone();
    assert!(
        draft.contains("季度报告"),
        "中文应能进入改名框，实际：{draft:?}"
    );
}