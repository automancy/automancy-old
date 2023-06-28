use std::fs;
use std::sync::Arc;

use ractor::concurrency::JoinHandle;
use ractor::{Actor, ActorRef};
use vulkano::device::DeviceExtensions;
use winit::event_loop::EventLoop;
use winit::window::{Icon, Window};

use automancy::camera::Camera;
use automancy::game::{Game, GameMsg, TICK_INTERVAL};
use automancy::gpu;
use automancy::gpu::{Gpu, RenderAlloc};
use automancy::map::{Map, MapInfo, MAIN_MENU, MAP_PATH};
use automancy_defs::cg::Double;
use automancy_defs::coord::ChunkCoord;
use automancy_defs::egui::Frame;
use automancy_defs::{egui, log};
use automancy_resources::kira::manager::backend::cpal::CpalBackend;
use automancy_resources::kira::manager::{AudioManager, AudioManagerSettings};
use automancy_resources::kira::track::{TrackBuilder, TrackHandle};
use automancy_resources::{ResourceManager, RESOURCES_PATH};

use crate::{gui, LOGO};

/// Stores what the game initializes on startup.
pub struct GameSetup {
    /// the audio manager
    pub audio_man: AudioManager,
    /// the resources manager
    pub resource_man: Arc<ResourceManager>,
    /// the game messaging system
    pub game: ActorRef<GameMsg>,
    /// the game's async handle, for graceful shutdown
    pub game_handle: Option<JoinHandle<()>>,
    /// the egui frame
    pub frame: Frame,
    /// the camera
    pub camera: Camera,
    /// the last camera position, in chunk coord
    pub camera_chunk_coord: ChunkCoord,
    /// the window
    pub window: Arc<Window>,
    /// the list of available maps
    pub maps: Vec<(MapInfo, String)>,

    pub map_gui_id: egui::Id,
}

impl GameSetup {
    /// Initializes the game, filling all the necessary fields as well as creating an event loop.
    pub async fn setup() -> (EventLoop<()>, Gpu, Self) {
        // --- resources & data ---
        let mut audio_man =
            AudioManager::<CpalBackend>::new(AudioManagerSettings::default()).unwrap();
        let track = audio_man
            .add_sub_track({
                let builder = TrackBuilder::new();

                builder
            })
            .unwrap();
        log::info!("audio backend initialized");
        let resource_man = load_resources(track);
        log::info!("loaded resources.");

        let icon = get_icon();

        // --- setup render ---
        let event_loop = EventLoop::new();

        let instance = gpu::create_instance();
        let window = gpu::create_window(icon, &event_loop);
        let surface = gpu::create_surface(window.clone(), instance.clone());

        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            khr_dedicated_allocation: true,
            khr_get_memory_requirements2: true,
            ..DeviceExtensions::default()
        };

        let (physical_device, queue_family_index) =
            gpu::get_physical_device(instance, surface.clone(), &device_extensions);
        log::info!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type
        );

        let (device, mut queues) = gpu::get_logical_device(
            physical_device.clone(),
            queue_family_index,
            device_extensions,
        );
        let queue = queues.next().unwrap();

        let alloc = RenderAlloc::new(
            &resource_man,
            device.clone(),
            surface.clone(),
            window.clone(),
            physical_device,
        );
        let gpu = Gpu::new(device, queue, surface, window.clone(), alloc);

        log::info!("Renderer setup complete");
        // --- setup game ---
        let (game, game_handle) =
            Actor::spawn(Some("game".to_string()), Game, resource_man.clone())
                .await
                .unwrap();

        game.send_message(GameMsg::LoadMap(
            resource_man.clone(),
            MAIN_MENU.to_string(),
        ))
        .unwrap();

        game.send_interval(TICK_INTERVAL, || GameMsg::Tick);

        log::info!("loading completed!");

        // last setup
        let frame = gui::default_frame();

        let size = window.inner_size();

        let camera = Camera::new(size.width as Double, size.height as Double);

        let map_gui_id = egui::Id::new("map_gui");

        // --- event-loop ---
        (
            event_loop,
            gpu,
            GameSetup {
                audio_man,
                resource_man,
                game,
                game_handle: Some(game_handle),
                frame,
                camera,
                camera_chunk_coord: camera.get_tile_coord().into(),
                window,
                maps: Vec::new(),

                map_gui_id,
            },
        )
    }
    /// Refreshes the list of maps on the filesystem. Should be done every time the list of maps could have changed (on map creation/delete and on game load).
    pub fn refresh_maps(&mut self) {
        drop(fs::create_dir_all(MAP_PATH));

        self.maps = fs::read_dir(MAP_PATH)
            .unwrap()
            .flatten()
            .map(|f| f.file_name().to_str().unwrap().to_string())
            .filter(|f| !f.starts_with('.'))
            .flat_map(|map| {
                Map::read_header(&self.resource_man, &map)
                    .map(|v| v.info)
                    .zip(Some(map))
            })
            .collect::<Vec<_>>();

        self.maps.sort_by(|a, b| a.1.cmp(&b.1));
        self.maps.sort_by(|a, b| a.0.save_time.cmp(&b.0.save_time));
        self.maps.reverse();
    }
}

/// Gets the game icon.
fn get_icon() -> Icon {
    let image = image::load_from_memory(LOGO).unwrap().to_rgba8();
    let width = image.width();
    let height = image.height();

    Icon::from_rgba(image.into_flat_samples().samples, width, height).unwrap()
}

/// Initialize the Resource Manager system, and loads all the resources in all namespaces.
fn load_resources(track: TrackHandle) -> Arc<ResourceManager> {
    let mut resource_man = ResourceManager::new(track);

    fs::read_dir(RESOURCES_PATH)
        .unwrap()
        .flatten()
        .map(|v| v.path())
        .for_each(|dir| {
            let namespace = dir.file_name().unwrap().to_str().unwrap();
            log::info!("loading namespace {namespace}");
            resource_man.load_models(&dir);
            resource_man.load_audios(&dir);
            resource_man.load_tiles(&dir);
            resource_man.load_items(&dir);
            resource_man.load_tags(&dir);
            resource_man.load_scripts(&dir);
            resource_man.load_translates(&dir);
            log::info!("finished loading namespace {namespace}");
        });

    resource_man.ordered_items();
    resource_man.compile_models();

    Arc::new(resource_man)
}
