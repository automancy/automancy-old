use std::error::Error;
use std::fs;
use std::sync::Arc;

use egui::Frame;
use ractor::concurrency::JoinHandle;
use ractor::{Actor, ActorRef};
use winit::window::Window;

use automancy::camera::Camera;
use automancy::game::{Game, GameMsg, TICK_INTERVAL};
use automancy::gpu::window_size_double;
use automancy::map::{Map, MapInfo, MAIN_MENU, MAP_PATH};
use automancy_defs::coord::ChunkCoord;
use automancy_defs::log;
use automancy_defs::rendering::Vertex;
use automancy_resources::kira::manager::backend::cpal::CpalBackend;
use automancy_resources::kira::manager::{AudioManager, AudioManagerSettings};
use automancy_resources::kira::track::{TrackBuilder, TrackHandle};
use automancy_resources::{ResourceManager, RESOURCES_PATH};

use crate::gui;

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
    /// the list of available maps
    pub maps: Vec<(MapInfo, String)>,
}

impl GameSetup {
    /// Initializes the game, filling all the necessary fields as well as returns the loaded vertices and indices.
    pub async fn setup(window: &Window) -> Result<(Self, Vec<Vertex>, Vec<u16>), Box<dyn Error>> {
        // --- resources & data ---
        log::info!("initializing audio backend...");
        let mut audio_man = AudioManager::<CpalBackend>::new(AudioManagerSettings::default())?;
        let track = audio_man.add_sub_track({
            let builder = TrackBuilder::new();

            builder
        })?;
        log::info!("audio backend initialized");

        log::info!("loading resources...");
        let (resource_man, vertices, indices) = load_resources(track);
        log::info!("loaded resources.");

        // --- game ---
        log::info!("creating game...");

        let (game, game_handle) =
            Actor::spawn(Some("game".to_string()), Game, resource_man.clone()).await?;

        game.send_message(GameMsg::LoadMap(
            resource_man.clone(),
            MAIN_MENU.to_string(),
        ))?;

        game.send_interval(TICK_INTERVAL, || GameMsg::Tick);

        log::info!("game created.");

        log::info!("loading completed!");

        // --- last setup ---
        let frame = gui::default_frame();

        let camera = Camera::new(window_size_double(window));

        // --- event-loop ---
        Ok((
            GameSetup {
                audio_man,
                resource_man,
                game,
                game_handle: Some(game_handle),
                frame,
                camera,
                camera_chunk_coord: camera.get_tile_coord().into(),
                maps: Vec::new(),
            },
            vertices,
            indices,
        ))
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

/// Initialize the Resource Manager system, and loads all the resources in all namespaces.
fn load_resources(track: TrackHandle) -> (Arc<ResourceManager>, Vec<Vertex>, Vec<u16>) {
    let mut resource_man = ResourceManager::new(track);

    fs::read_dir(RESOURCES_PATH)
        .unwrap()
        .flatten()
        .map(|v| v.path())
        .for_each(|dir| {
            let namespace = dir.file_name().unwrap().to_str().unwrap();
            log::info!("loading namespace {namespace}...");
            resource_man.load_models(&dir);
            resource_man.load_audios(&dir);
            resource_man.load_tiles(&dir);
            resource_man.load_items(&dir);
            resource_man.load_tags(&dir);
            resource_man.load_scripts(&dir);
            resource_man.load_translates(&dir);
            log::info!("loaded namespace {namespace}.");
        });

    resource_man.ordered_items();
    let (vertices, indices) = resource_man.compile_models();

    (Arc::new(resource_man), vertices, indices)
}
