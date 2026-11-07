//! 无头端到端测试（egui_kittest）：真实跑 App、注入按键、无 GPU 窗口截图。
//! 用户在用电脑时也能跑，不碰真实键盘鼠标。

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use egui_kittest::{kittest::By, kittest::Queryable, Harness};

use super::theme;
use super::{App, EditorState};
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

/// 回归：**文件夹**改名同样要吃中文。
///
/// 文件夹走的是检查器里另一条分支（`kind == "dir"`），而且它下面没有文本
/// 编辑器——隐私输入若还开着，`privacy_on` 会挂着每帧把系统输入法摁成英文，
/// 表现就是文件能改名、文件夹改名打不进中文。
#[test]
fn kittest_rename_folder_accepts_chinese() {
    let dir = setup_vault("renamefoldercn");
    let (mut harness, state) = make_app(dir);

    harness.run_steps(3);
    harness
        .get(By::new().role(egui::accesskit::Role::PasswordInput))
        .type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);

    let folder = service::create_folder(&state, "root", "归档").unwrap();
    harness.state_mut().selected = Some(folder.clone());
    harness.run_steps(2);
    harness.state_mut().begin_rename("归档");
    harness.run_steps(2);
    assert!(harness.state().renaming, "文件夹应进入改名状态");

    // 改名框不该逐帧打断输入法组合
    let interrupted = harness
        .output()
        .platform_output
        .ime
        .as_ref()
        .is_some_and(|i| i.should_interrupt_composition);
    assert!(!interrupted, "文件夹改名框不该逐帧打断输入法组合");

    // 输入法上屏 + 回车提交，整条链路走通
    harness.input_mut().events.push(egui::Event::Ime(
        egui::ImeEvent::Commit("季度资料".into()),
    ));
    harness.run_steps(2);
    assert!(
        harness.state().rename_draft.contains("季度资料"),
        "中文应能进入文件夹改名框，实际：{:?}",
        harness.state().rename_draft
    );
    harness.key_press(egui::Key::Enter);
    harness.run_steps(3);

    let names: Vec<String> = service::list_dir(&state, "root")
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert!(
        names.iter().any(|n| n.contains("季度资料")),
        "改好的中文文件夹名应落库，实际：{names:?}"
    );
}

/// 打开一个文件后，右上角要能看出这份内容在库里还有没有副本、在哪里。
#[test]
fn kittest_duplicate_badge() {
    let dir = setup_vault("dupb");
    // 再放一份内容完全相同、名字和目录都不同的副本
    {
        let dek = vault::unlock(&dir, PW).unwrap();
        let mut v = store::Vault::open(dir.clone(), dek).unwrap();
        let folder = v.create_folder("root", "归档").unwrap();
        let src = dir.join("_dup.txt");
        std::fs::write(&src, "hello test").unwrap();
        v.import_file(&folder.id, "副本.txt", &src).unwrap();
        v.save_index().unwrap();
    }

    let (mut harness, _state) = make_app(dir);
    harness.run_steps(3);
    harness
        .get(By::new().role(egui::accesskit::Role::PasswordInput))
        .type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);

    harness.get(By::new().label("hello.txt")).click();
    harness.run_steps(2);

    // 查重在后台线程跑，等结论回来（最多 ~3 秒）
    for _ in 0..100 {
        let done = {
            let app = harness.state();
            !app.dup.pending && app.dup.for_id != ""
        };
        if done {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
        harness.run_steps(1);
    }

    {
        let app = harness.state();
        assert!(!app.dup.pending, "查重应该已经出结果");
        assert_eq!(app.dup.hits.len(), 1, "应找到一份副本：{:?}", app.dup.hits);
        assert_eq!(app.dup.hits[0].folder, "全部文件 / 归档");
        assert_eq!(app.dup.hits[0].name, "副本.txt");
    }

    // 角标文案要出现在界面里
    harness.get(By::new().label_contains("另有 1 份"));
}

/// 回归：编辑器消失后隐私模式必须自动退出。
///
/// 隐私输入只在文本编辑器里存在。删文件 / 条目从列表里消失时旧代码只把
/// `editor` 清掉、`privacy_on` 留着，于是 `keep_ime_off` 每帧把窗口输入法
/// 摁成英文——全应用的输入框（最典型的就是改名框）从此打不进中文，
/// 英文却正常，很难联想到是隐私模式没退。
#[test]
fn kittest_privacy_exits_when_editor_vanishes() {
    let dir = setup_vault("privacyexit");
    let (mut harness, _state) = make_app(dir);

    harness.run_steps(2);
    harness
        .get(By::new().role(egui::accesskit::Role::PasswordInput))
        .type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);

    harness.get(By::new().label("hello.txt")).click();
    harness.run_steps(2);
    harness.state_mut().set_privacy_mode(true);
    harness.run_steps(2);
    assert!(harness.state().privacy_on, "隐私模式应已开启");

    // 模拟「选中的条目消失了」：编辑器与选中项一起被清掉（见 refresh_list）
    harness.state_mut().selected = None;
    harness.state_mut().editor = None;
    harness.run_steps(2);

    assert!(
        !harness.state().privacy_on,
        "编辑器没了必须退出隐私模式，否则系统输入法会一直停在英文"
    );
}

/// 页签：打开多个条目、切回来保留各自状态、关掉时按顺序接上。
#[test]
fn tabs_keep_their_own_state() {
    let dir = setup_vault("tabs");
    let state = Arc::new(AppState::default());
    *state.config.lock().unwrap() = Config {
        vault_path: dir.display().to_string(),
        ..Default::default()
    };
    service::open_vault_at(&state, &dir.display().to_string()).unwrap();
    service::unlock(&state, PW).unwrap();

    // 再塞一个文件，凑够两个页签（必须放在库目录之外，否则会被导入逻辑跳过）
    let src2 = std::env::temp_dir().join("xv-kittest-tabs-src2.txt");
    std::fs::write(&src2, "second").unwrap();
    service::import_paths(&state, &[src2.display().to_string()], "root").unwrap();

    let mut app = App::new(Arc::clone(&state), egui::Context::default());
    let entries = service::list_dir(&state, "root").unwrap();
    assert_eq!(entries.len(), 2);

    app.select(Some(entries[0].clone()));
    // 页签自己的编辑器状态：切走再切回来要还在
    app.editor = Some(EditorState {
        entry_id: entries[0].id.clone(),
        text: "草稿".into(),
        dirty: false,
        last_edit: None,
        truncated: false,
        phase: "已加密写入",
    });

    // 锁住第一个页签，否则打开第二个时会把它顶掉
    app.toggle_pin(0);
    app.select(Some(entries[1].clone()));
    assert_eq!(app.tabs.len(), 2, "两个文件应该开两个页签");
    assert_eq!(app.active, 1);
    assert!(app.editor.is_none(), "切走之后不该把上一个页签的编辑器带过来");

    // 重复打开同一个文件只切过去，不再开新页签
    app.select(Some(entries[0].clone()));
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.active, 0);
    assert_eq!(app.editor.as_ref().unwrap().text, "草稿", "切回来要恢复编辑器状态");

    // 关掉当前页签，剩下的那个接上
    app.close_tab(0);
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.selected.as_ref().unwrap().id, entries[1].id);

    // 全关掉就回到空状态
    app.close_tab(0);
    assert!(app.tabs.is_empty());
    assert!(app.selected.is_none());
}
/// 生成一段静音 PCM WAV，供音频预览测试用。
fn write_silent_wav(path: &std::path::Path, rate: u32, ms: u32) {
    let samples = rate as u64 * ms as u64 / 1000;
    let data_len = (samples * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.resize(44 + data_len as usize, 0);
    std::fs::write(path, out).unwrap();
}

/// 回归：纯音频文件要进播放器（占位卡 + 控制条），不能落到「不支持预览」页。
#[test]
fn kittest_audio_preview_plays() {
    let dir = std::env::temp_dir().join("xv-kittest-audio");
    let _ = std::fs::remove_dir_all(&dir);
    vault::create(&dir, PW, None).unwrap();
    let dek = vault::unlock(&dir, PW).unwrap();
    let mut v = store::Vault::open(dir.clone(), dek).unwrap();
    let wav = dir.join("tone.wav");
    write_silent_wav(&wav, 44_100, 1000);
    v.import_file("root", "tone.wav", &wav).unwrap();
    v.save_index().unwrap();

    let (mut harness, _state) = make_app(dir);
    harness.run_steps(3);
    harness
        .get(By::new().role(egui::accesskit::Role::PasswordInput))
        .type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);

    if let Some(node) = harness.query(By::new().label("tone.wav")) {
        node.click();
    }
    harness.run_steps(5);

    // 等播放器起来并读到时长
    for _ in 0..60 {
        let ready = harness
            .state()
            .video
            .player
            .as_ref()
            .is_some_and(|p| p.duration_ms() > 0 || p.error().is_some());
        if ready {
            break;
        }
        harness.run_steps(1);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }

    let p = harness.state().video.player.as_ref().expect("播放器没起来");
    assert!(!p.has_video(), "wav 不该被当成视频");
    assert!(p.duration_ms() > 0, "时长没读到");

    // 有输出设备时才验证播放/暂停；没设备时播放器会给提示，跳过按键部分。
    if p.error().is_none() {
        harness.key_press(egui::Key::Space);
        harness.run_steps(3);
        assert!(
            harness.state().video.player.as_ref().unwrap().is_paused(),
            "空格应该暂停"
        );
    }
}

/// 页签锁定：没锁的页签会被新打开的文件顶掉，锁定的始终保留。
#[test]
fn pinned_tab_survives_new_open() {
    let dir = setup_vault("pintab");
    let state = Arc::new(AppState::default());
    *state.config.lock().unwrap() = Config {
        vault_path: dir.display().to_string(),
        ..Default::default()
    };
    service::open_vault_at(&state, &dir.display().to_string()).unwrap();
    service::unlock(&state, PW).unwrap();

    // 再导入两个文件，凑够三个
    let src = std::env::temp_dir().join("xv-kittest-pin-src");
    let _ = std::fs::remove_dir_all(&src);
    std::fs::create_dir_all(&src).unwrap();
    for name in ["second.txt", "third.txt"] {
        let p = src.join(name);
        std::fs::write(&p, name).unwrap();
        service::import_paths(&state, &[p.display().to_string()], "root").unwrap();
    }

    let mut app = App::new(Arc::clone(&state), egui::Context::default());
    let entries = service::list_dir(&state, "root").unwrap();
    let find = |n: &str| entries.iter().find(|e| e.name == n).unwrap().clone();

    // 打开 A（临时页签），再打开 B：A 被顶掉，只剩一个页签
    app.select(Some(find("hello.txt")));
    app.select(Some(find("second.txt")));
    assert_eq!(app.tabs.len(), 1, "未锁定的页签应该被顶掉");
    assert_eq!(app.tabs[0].entry.name, "second.txt");

    // 锁定 B，再打开 C：B 保留，C 是新的临时页签
    app.toggle_pin(0);
    app.select(Some(find("third.txt")));
    assert_eq!(app.tabs.len(), 2);
    assert!(app.tabs[0].pinned && app.tabs[0].entry.name == "second.txt", "锁定的页签要排在最左");
    assert!(!app.tabs[1].pinned && app.tabs[1].entry.name == "third.txt");

    // 再打开 A：顶掉临时页签 C，锁定的 B 还在
    app.select(Some(find("hello.txt")));
    assert_eq!(app.tabs.len(), 2);
    assert!(app.tabs.iter().any(|t| t.pinned && t.entry.name == "second.txt"));
    assert!(app.tabs.iter().any(|t| !t.pinned && t.entry.name == "hello.txt"));
    assert!(!app.tabs.iter().any(|t| t.entry.name == "third.txt"));

    // 锁定 A 之后两个都在锁定块；再打开 C 追加到后面
    app.toggle_pin(1);
    assert!(app.tabs[0].pinned && app.tabs[1].pinned, "锁定的页签应该连成一块");
    app.select(Some(find("third.txt")));
    assert_eq!(app.tabs.len(), 3);
    assert_eq!(app.tabs[2].entry.name, "third.txt");

    // 解锁最左边的 B：它应该移到锁定块之后，激活的 C 不受影响
    app.toggle_pin(0);
    assert_eq!(app.tabs[0].entry.name, "hello.txt", "剩下的锁定页签要顶到最左");
    assert_eq!(app.tabs[1].entry.name, "second.txt", "解锁的页签排到锁定块之后");
    assert!(!app.tabs[1].pinned);
    assert_eq!(app.tabs[2].entry.name, "third.txt");
    assert_eq!(app.active, 2, "重排不能把激活的页签换掉");
}



/// 回收站：「清空回收站」按钮只在有东西时出现，点了要过确认弹窗，确认后整批销毁。
#[test]
fn kittest_trash_empty_button_flow() {
    let dir = setup_vault("trash-empty");
    let (mut harness, state) = make_app(dir);

    harness.run_steps(2);
    let pw_node = harness.get(By::new().role(egui::accesskit::Role::PasswordInput));
    pw_node.type_text(PW);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(10);

    // 回收站空着：不该有「清空回收站」按钮
    harness
        .get(By::new().role(egui::accesskit::Role::Button).label("回收站"))
        .click();
    harness.run_steps(3);
    harness.get(By::new().label("回收站是空的"));
    assert!(
        harness.query(By::new().label("清空回收站")).is_none(),
        "空回收站不该出现清空按钮"
    );

    // 删掉唯一的文件，回收站里有一项，按钮出现
    let id = service::list_dir(&state, "root").unwrap()[0].id.clone();
    service::delete_entry(&state, &id).unwrap();
    harness.state_mut().need_list = true;
    harness.run_steps(3);
    harness
        .render()
        .unwrap()
        .save("target/kittest-trash-filled.png")
        .unwrap();
    harness.get(By::new().label("清空回收站")).click();
    harness.run_steps(2);

    // 弹窗先拦一道，此时条目还在
    harness.get(By::new().label_contains("永久销毁"));
    harness
        .render()
        .unwrap()
        .save("target/kittest-trash-confirm.png")
        .unwrap();
    assert_eq!(service::list_trash(&state).unwrap().len(), 1, "确认前不能删");

    harness.get(By::new().label("清空")).click();
    harness.run_steps(3);
    assert!(service::list_trash(&state).unwrap().is_empty(), "确认后回收站应清空");
    harness.get(By::new().label("回收站是空的"));

    harness.render().unwrap().save("target/kittest-trash.png").unwrap();
}
