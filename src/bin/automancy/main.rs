#![windows_subsystem = "windows"]

use std::fmt::Write;
use std::fs::File;
use std::panic::PanicInfo;
use std::path::Path;
use std::time::{Duration, Instant};
use std::{env, panic};

use color_eyre::config::HookBuilder;
use color_eyre::eyre;
use color_eyre::owo_colors::OwoColorize;
use egui::{ColorImage, TextureHandle, TextureOptions};
use env_logger::Env;
use futures::executor::block_on;
use native_dialog::{MessageDialog, MessageType};
use num::Zero;
use tokio::runtime::Runtime;
use uuid::Uuid;
use winit::dpi::PhysicalSize;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, Icon, WindowBuilder};

use automancy::camera::Camera;
use automancy::gpu::Gpu;
use automancy_defs::flexstr::ToSharedStr;
use automancy_defs::gui::init_gui;
use automancy_defs::gui::set_font;
use automancy_defs::math::Double;
use automancy_defs::{log, window};
use automancy_resources::kira::tween::Tween;

use crate::event::{on_event, EventLoopStorage};
use crate::gui::init_fonts;
use crate::renderer::Renderer;
use crate::setup::GameSetup;

pub static LOGO_PATH: &str = "assets/logo_256.png";
pub static LOGO: &[u8] = include_bytes!("assets/logo_256.png");

mod event;
mod gui;
pub mod renderer;
mod setup;

/// Gets the game icon.
fn get_icon_and_image(context: &egui::Context) -> (Icon, TextureHandle) {
    let image = image::load_from_memory(LOGO).unwrap().to_rgba8();
    let width = image.width();
    let height = image.height();

    let samples = image.into_flat_samples().samples;
    let texture = context.load_texture(
        LOGO_PATH.to_string(),
        ColorImage::from_rgba_premultiplied([width as usize, height as usize], &samples),
        TextureOptions::LINEAR,
    );

    (Icon::from_rgba(samples, width, height).unwrap(), texture)
}

fn write_msg<P: AsRef<Path>>(buffer: &mut impl Write, file_path: P) -> std::fmt::Result {
    writeln!(buffer, "Well, this is embarrassing.\n")?;
    writeln!(
        buffer,
        "automancy had a problem and crashed. To help us diagnose the problem you can send us a crash report.\n"
    )?;
    writeln!(
        buffer,
        "We have generated a report file at\nfile://{}\n\nSubmit an issue or tag us on Fedi/Discord and include the report as an attachment.\n",
        file_path.as_ref().display(),
    )?;

    writeln!(buffer, "- Git: https://github.com/automancy/automancy")?;
    writeln!(buffer, "- Fedi(Mastodon): https://gamedev.lgbt/@automancy")?;
    writeln!(buffer, "- Discord: https://discord.gg/ee9XebxNaa")?;

    writeln!(
        buffer,
        "\nAlternatively, send an email to the main developer Madeline Sparkles (madeline@mouse.lgbt) directly.\n"
    )?;

    writeln!(
        buffer,
        "We take privacy seriously, and do not perform any automated error collection. In order to improve the software, we rely on people to submit reports.\n"
    )?;
    writeln!(buffer, "Thank you kindly!")?;

    Ok(())
}
fn main() -> eyre::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    {
        let eyre = HookBuilder::blank()
            .capture_span_trace_by_default(true)
            .display_env_section(false);

        let (panic_hook, eyre_hook) = eyre.into_hooks();

        eyre_hook.install()?;

        panic::set_hook(Box::new(move |info: &PanicInfo| {
            let file_path = {
                let report = panic_hook.panic_report(info);

                let uuid = Uuid::new_v4().hyphenated().to_string();
                let tmp_dir = env::temp_dir();
                let file_name = format!("automancy-report-{uuid}.txt");
                let file_path = tmp_dir.join(file_name);
                if let Ok(mut file) = File::create(&file_path) {
                    use std::io::Write;

                    _ = write!(
                        file,
                        "{}",
                        strip_ansi_escapes::strip_str(report.to_string())
                    );
                }
                eprintln!("{}", report);

                file_path
            };

            if let Some(location) = info.location() {
                if !["src/game.rs", "src/tile_entity.rs"].contains(&location.file()) {
                    let message = {
                        let mut message = String::new();
                        _ = write_msg(&mut message, &file_path);

                        message
                    };

                    {
                        eprintln!("\n\n\n{}\n\n\n", message.bright_red());

                        _ = MessageDialog::new()
                            .set_type(MessageType::Error)
                            .set_title("automancy crash dialog")
                            .set_text(&message)
                            .show_alert();
                    }
                }
            }
        }));
    }

    // --- window ---
    let event_loop = EventLoop::new()?;

    let egui_context: egui::Context = Default::default();

    let (icon, logo_image) = get_icon_and_image(&egui_context);

    let window = WindowBuilder::new()
        .with_title("automancy")
        .with_window_icon(Some(icon))
        .with_min_inner_size(PhysicalSize::new(200, 200))
        .build(&event_loop)
        .expect("Failed to open window");

    let camera = Camera::new(window::window_size_double(&window));

    // --- setup ---
    let runtime = Runtime::new().unwrap();

    let (mut setup, vertices, indices) = runtime
        .block_on(GameSetup::setup(camera, logo_image))
        .expect("Critical failure in game setup");

    // --- render ---
    log::info!("Setting up rendering...");
    let gpu = block_on(Gpu::new(
        window,
        &setup.resource_man,
        vertices,
        indices,
        setup.options.graphics.fps_limit == 0.0,
    ));
    log::info!("Render setup.");

    // --- gui ---
    log::info!("Setting up gui...");
    let mut gui = init_gui(
        egui_context,
        egui_wgpu::Renderer::new(&gpu.device, gpu.config.format, None, 1),
        &gpu.window,
    );
    init_fonts(setup.resource_man.clone(), &mut gui);
    set_font(setup.options.gui.font.to_shared_str(), &mut gui);
    log::info!("Gui set up.");

    let mut renderer = Renderer::new(gpu, &setup.options);

    let mut loop_store = EventLoopStorage::default();

    let mut closed = false;

    event_loop.run(move |event, target| {
        if closed {
            return;
        }

        match on_event(
            &mut setup,
            &mut loop_store,
            &mut renderer,
            &mut gui,
            event,
            target,
        ) {
            Ok(to_exit) => {
                if to_exit {
                    closed = true;
                    return;
                }
            }
            Err(e) => {
                log::warn!("Event loop returned error: {e}");
            }
        }

        if !setup.options.synced {
            gui.context.set_zoom_factor(setup.options.gui.scale);
            set_font(setup.options.gui.font.to_shared_str(), &mut gui);

            setup
                .audio_man
                .main_track()
                .set_volume(setup.options.audio.sfx_volume, Tween::default())
                .unwrap();

            renderer
                .gpu
                .set_vsync(setup.options.graphics.fps_limit == 0.0);

            if setup.options.graphics.fps_limit >= 250.0 {
                renderer.fps_limit = Double::INFINITY;
            } else {
                renderer.fps_limit = setup.options.graphics.fps_limit;
            }

            if setup.options.graphics.fullscreen {
                renderer
                    .gpu
                    .window
                    .set_fullscreen(Some(Fullscreen::Borderless(None)));
            } else {
                renderer.gpu.window.set_fullscreen(None);
            }

            setup.options.synced = true;
        }

        if !renderer.fps_limit.is_zero() {
            let frame_time = Duration::from_secs_f64(1.0 / renderer.fps_limit);

            if loop_store.frame_start.elapsed() > frame_time {
                renderer.gpu.window.request_redraw();
                target.set_control_flow(ControlFlow::WaitUntil(Instant::now() + frame_time));
            }
        } else {
            renderer.gpu.window.request_redraw();
            target.set_control_flow(ControlFlow::Poll);
        }

        loop_store.elapsed = Instant::now().duration_since(loop_store.frame_start);
    })?;

    Ok(())
}
