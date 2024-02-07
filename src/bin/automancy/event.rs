use std::collections::VecDeque;
use std::mem;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{LayerId, Rect};
use egui_wgpu::wgpu::SurfaceError;
use fuse_rust::Fuse;
use futures::channel::mpsc;
use futures::executor::block_on;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;

use automancy::game::{GameMsg, PlaceTileResponse};
use automancy::gpu::AnimationMap;
use automancy::input;
use automancy::input::KeyActions;
use automancy::tile_entity::TileEntityMsg;
use automancy_defs::colors::ColorAdj;
use automancy_defs::coord::TileCoord;
use automancy_defs::glam::{dvec2, vec3};
use automancy_defs::gui::Gui;
use automancy_defs::hashbrown::{HashMap, HashSet};
use automancy_defs::id::Id;
use automancy_defs::math::{Float, Matrix4, FAR, HEX_GRID_LAYOUT};
use automancy_defs::rendering::{make_line, InstanceData};
use automancy_defs::{colors, log, math, window};
use automancy_resources::data::item::Item;
use automancy_resources::data::Data;

use crate::gui;
use crate::gui::{
    error, info, menu, player, popup, tile_config, tile_selection, GameEguiCallback, GuiState,
    PopupState, Screen, TextField,
};
use crate::renderer::Renderer;
use crate::setup::GameSetup;

/// Stores information that lives for the entire lifetime of the session, and is not dropped at the end of one event cycle or handled elsewhere.
pub struct EventLoopStorage {
    /// fuzzy search engine
    pub fuse: Fuse,
    /// the currently selected tile.
    pub selected_tile_id: Option<Id>,
    /// the last placed tile, to prevent repeatedly sending place requests
    pub already_placed_at: Option<TileCoord>,
    /// the tile that has its config menu open.
    pub config_open: Option<TileCoord>,
    /// tag searching cache
    pub tag_cache: HashMap<Id, Arc<Vec<Item>>>,
    /// tile currently linking
    pub linking_tile: Option<TileCoord>,
    /// the last frame's starting time
    pub frame_start: Instant,
    /// the elapsed time between each frame
    pub elapsed: Duration,
    /// the currently grouped tiles
    pub grouped_tiles: HashSet<TileCoord>,
    /// the stored initial cursor position, for moving tiles
    pub initial_cursor_position: Option<TileCoord>,

    pub take_item_animations: HashMap<Item, VecDeque<(Instant, Rect)>>,

    pub gui_state: GuiState,
}

impl Default for EventLoopStorage {
    fn default() -> Self {
        Self {
            fuse: Fuse::default(),
            selected_tile_id: None,
            already_placed_at: None,
            config_open: None,
            tag_cache: Default::default(),
            linking_tile: None,
            frame_start: Instant::now(),
            elapsed: Default::default(),
            grouped_tiles: Default::default(),
            initial_cursor_position: None,
            take_item_animations: Default::default(),

            gui_state: Default::default(),
        }
    }
}

impl EventLoopStorage {}

pub fn shutdown_graceful(
    setup: &mut GameSetup,
    target: &EventLoopWindowTarget<()>,
) -> anyhow::Result<bool> {
    setup.game.send_message(GameMsg::StopTicking)?;

    block_on(setup.game.call(
        |reply| GameMsg::SaveMap(setup.resource_man.clone(), reply),
        None,
    ))
    .unwrap();

    setup.game.stop(Some("Game closed".to_string()));

    block_on(setup.game_handle.take().unwrap())?;

    target.exit();

    log::info!("Shut down gracefully");

    Ok(true)
}

fn render(
    setup: &mut GameSetup,
    loop_store: &mut EventLoopStorage,
    renderer: &mut Renderer,
    gui: &mut Gui,
    target: &EventLoopWindowTarget<()>,
) -> anyhow::Result<bool> {
    gui::reset_callback_counter();

    let mut result = Ok(false);

    setup.camera.update_pointing_at(
        setup.input_handler.main_pos,
        window::window_size_double(&renderer.gpu.window),
    );

    setup.camera.update_pos(
        window::window_size_double(&renderer.gpu.window),
        loop_store.elapsed.as_secs_f64(),
    );

    let mut tile_tints = HashMap::new();

    let mut extra_instances = vec![];
    let mut in_world_item_instances = vec![];

    gui.renderer.callback_resources.insert(AnimationMap::new());

    let camera_pos_float = setup.camera.get_pos().as_vec3();

    let (selection_send, mut selection_recv) = mpsc::channel(1);

    loop_store.frame_start = Instant::now();

    {
        gui.context
            .begin_frame(gui.state.take_egui_input(&renderer.gpu.window));

        #[cfg(debug_assertions)]
        if setup.input_handler.key_active(KeyActions::Debug) {
            use crate::gui::debug;
            gui.context.set_debug_on_hover(true);

            debug::debugger(setup, loop_store, renderer, &gui.context);
        } else {
            gui.context.set_debug_on_hover(false);
        }

        if loop_store.gui_state.popup == PopupState::None {
            match loop_store.gui_state.screen {
                Screen::Ingame => {
                    if !setup.input_handler.key_active(KeyActions::HideGui) {
                        let mut game_data = block_on(setup.game.call(GameMsg::TakeDataMap, None))
                            .unwrap()
                            .unwrap();

                        if setup.input_handler.key_active(KeyActions::Player) {
                            player::player(setup, loop_store, &gui.context, &game_data);
                        }

                        // tile_info
                        info::info(setup, &gui.context);

                        // tile_config
                        tile_config::tile_config(setup, loop_store, &gui.context, &mut game_data);

                        // tile_selections
                        tile_selection::tile_selections(
                            setup,
                            loop_store,
                            &gui.context,
                            selection_send,
                            &game_data,
                        );

                        if let Ok(Some(id)) = selection_recv.try_next() {
                            loop_store.already_placed_at = None;

                            if loop_store.selected_tile_id == Some(id) {
                                loop_store.selected_tile_id = None;
                            } else {
                                loop_store.selected_tile_id = Some(id);
                            }
                        }

                        let cursor_pos = math::screen_to_world(
                            window::window_size_double(&renderer.gpu.window),
                            setup.input_handler.main_pos,
                            setup.camera.get_pos(),
                        );
                        let cursor_pos = dvec2(cursor_pos.x, cursor_pos.y);

                        if let Some(tile) = loop_store
                            .selected_tile_id
                            .and_then(|id| setup.resource_man.registry.tiles.get(&id))
                        {
                            gui.context.layer_painter(LayerId::background()).add(
                                egui_wgpu::Callback::new_paint_callback(
                                    gui.context.screen_rect(),
                                    GameEguiCallback::new(
                                        InstanceData::default()
                                            .with_alpha(0.6)
                                            .with_light_pos(camera_pos_float, None)
                                            .with_projection(setup.camera.get_matrix().as_mat4())
                                            .with_model_matrix(Matrix4::from_translation(vec3(
                                                cursor_pos.x as Float,
                                                cursor_pos.y as Float,
                                                FAR as Float,
                                            ))),
                                        tile.model,
                                        gui.context.screen_rect(),
                                        gui.context.screen_rect(),
                                    ),
                                ),
                            );
                        }

                        if let Some(coord) = loop_store.linking_tile {
                            extra_instances.push((
                                InstanceData::default()
                                    .with_color_offset(colors::RED.to_array())
                                    .with_light_pos(camera_pos_float, None)
                                    .with_model_matrix(make_line(
                                        HEX_GRID_LAYOUT.hex_to_world_pos(*coord),
                                        cursor_pos.as_vec2(),
                                    )),
                                setup.resource_man.registry.model_ids.cube1x1,
                            ));
                        }

                        setup
                            .game
                            .send_message(GameMsg::SetDataMap(game_data))
                            .unwrap();
                    }
                }
                Screen::MainMenu => {
                    result = menu::main_menu(setup, &gui.context, target, loop_store)
                }
                Screen::MapLoad => {
                    menu::map_menu(setup, &gui.context, loop_store);
                }
                Screen::Options => {
                    menu::options_menu(setup, &gui.context, loop_store);
                }
                Screen::Paused => {
                    menu::pause_menu(setup, &gui.context, loop_store);
                }
                Screen::Research => {}
            }
        }
        match loop_store.gui_state.popup.clone() {
            PopupState::None => {}
            PopupState::MapCreate => popup::map_create_popup(setup, gui, loop_store),
            PopupState::MapDeleteConfirmation(map_name) => {
                popup::map_delete_popup(setup, gui, loop_store, &map_name);
            }
            PopupState::InvalidName => {
                popup::invalid_name_popup(setup, gui, loop_store);
            }
        }

        tile_tints.insert(setup.camera.pointing_at, colors::RED.with_alpha(0.2));

        for coord in &loop_store.grouped_tiles {
            tile_tints.insert(*coord, colors::ORANGE.with_alpha(0.4));
        }

        if setup.input_handler.control_held {
            if let Some(start) = loop_store.initial_cursor_position {
                let direction = setup.camera.pointing_at - start;

                if start != setup.camera.pointing_at {
                    extra_instances.push((
                        InstanceData::default()
                            .with_color_offset(colors::LIGHT_BLUE.to_array())
                            .with_light_pos(camera_pos_float, None)
                            .with_model_matrix(make_line(
                                HEX_GRID_LAYOUT.hex_to_world_pos(*start),
                                HEX_GRID_LAYOUT.hex_to_world_pos(*setup.camera.pointing_at),
                            )),
                        setup.resource_man.registry.model_ids.cube1x1,
                    ));
                }

                for coord in &loop_store.grouped_tiles {
                    let dest = *coord + direction;
                    tile_tints.insert(dest, colors::LIGHT_BLUE.with_alpha(0.3));
                }
            }
        }

        error::error_popup(setup, gui);

        if !matches!(result, Ok(true)) {
            match renderer.render(
                setup,
                gui,
                tile_tints,
                extra_instances,
                in_world_item_instances,
            ) {
                Ok(_) => {}
                Err(SurfaceError::Lost) => renderer.gpu.resize(
                    &mut renderer.shared_resources,
                    &mut renderer.render_resources,
                    renderer.gpu.window.inner_size(),
                ),
                Err(SurfaceError::OutOfMemory) => {
                    return shutdown_graceful(setup, target);
                }
                Err(e) => log::error!("{e:?}"),
            }
        }
    }

    result
}

fn on_link_tile(setup: &mut GameSetup, linking_tile: TileCoord) {
    let Some(id) = block_on(setup.game.call(
        |reply| GameMsg::GetTile(setup.camera.pointing_at, reply),
        None,
    ))
    .unwrap()
    .unwrap() else {
        return;
    };

    let Some(entity) = block_on(setup.game.call(
        |reply| GameMsg::GetTileEntity(setup.camera.pointing_at, reply),
        None,
    ))
    .unwrap()
    .unwrap() else {
        return;
    };

    let Some(tile) = setup.resource_man.registry.tiles.get(&id) else {
        return;
    };

    if tile
        .data
        .get(&setup.resource_man.registry.data_ids.linked)
        .cloned()
        .and_then(Data::into_bool)
        .unwrap_or(false)
    {
        let old = block_on(entity.call(
            |reply| TileEntityMsg::GetDataValue(setup.resource_man.registry.data_ids.link, reply),
            None,
        ))
        .unwrap()
        .unwrap();

        if old.is_some() {
            entity
                .send_message(TileEntityMsg::RemoveData(
                    setup.resource_man.registry.data_ids.link,
                ))
                .unwrap();

            setup
                .audio_man
                .play(setup.resource_man.audio["click"].clone())
                .unwrap();
            // TODO click2
        } else {
            entity
                .send_message(TileEntityMsg::SetDataValue(
                    setup.resource_man.registry.data_ids.link,
                    Data::Coord(linking_tile),
                ))
                .unwrap();

            setup
                .audio_man
                .play(setup.resource_man.audio["click"].clone())
                .unwrap();
        }
    }
}

/// Triggers every time the event loop is run once.
pub fn on_event(
    setup: &mut GameSetup,
    loop_store: &mut EventLoopStorage,
    renderer: &mut Renderer,
    gui: &mut Gui,
    event: Event<()>,
    target: &EventLoopWindowTarget<()>,
) -> anyhow::Result<bool> {
    let mut window_event = None;
    let mut device_event = None;

    let resource_man = setup.resource_man.clone();
    match &event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => {
            // game shutdown
            return shutdown_graceful(setup, target);
        }

        Event::WindowEvent { event, window_id } if window_id == &renderer.gpu.window.id() => {
            if !gui
                .state
                .on_window_event(&renderer.gpu.window, event)
                .consumed
            {
                window_event = Some(event);
            }

            match event {
                WindowEvent::RedrawRequested => {
                    renderer.gpu.window.pre_present_notify();

                    return render(setup, loop_store, renderer, gui, target);
                }
                WindowEvent::Resized(size) => {
                    renderer.gpu.resize(
                        &mut renderer.shared_resources,
                        &mut renderer.render_resources,
                        *size,
                    );

                    return Ok(false);
                }
                _ => {}
            }
        }

        Event::DeviceEvent { event, .. } => {
            device_event = Some(event);
        }

        _ => {}
    };

    if window_event.is_some() || device_event.is_some() {
        setup.input_handler.reset();
        setup.input_handler.update(input::convert_input(
            window_event,
            device_event,
            window::window_size_double(&renderer.gpu.window),
            1.0, //TODO sensitivity option
        ));

        let ignore_move = loop_store.selected_tile_id.is_some();

        setup.camera.handle_input(&setup.input_handler, ignore_move);

        if setup.input_handler.key_active(KeyActions::Escape) {
            // one by one
            if loop_store.selected_tile_id.take().is_none()
                && loop_store.linking_tile.take().is_none()
            {
                if loop_store
                    .gui_state
                    .switch_screen_when(&|s| s.screen == Screen::Ingame, Screen::Paused)
                {
                    block_on(setup.game.call(
                        |reply| GameMsg::SaveMap(setup.resource_man.clone(), reply),
                        None,
                    ))?
                    .unwrap();
                } else {
                    loop_store
                        .gui_state
                        .switch_screen_when(&|s| s.screen == Screen::Paused, Screen::Ingame);
                }

                if loop_store.gui_state.screen == Screen::Research {
                    loop_store.gui_state.return_screen();
                }
            }
        }

        if setup.input_handler.main_pressed
            || (setup.input_handler.shift_held && setup.input_handler.main_held)
        {
            if let Some(id) = loop_store.selected_tile_id {
                if loop_store.already_placed_at != Some(setup.camera.pointing_at) {
                    let response = block_on(setup.game.call(
                        |reply| GameMsg::PlaceTile {
                            coord: setup.camera.pointing_at,
                            id,
                            record: true,
                            reply: Some(reply),
                            data: None,
                        },
                        None,
                    ))?
                    .unwrap();

                    match response {
                        PlaceTileResponse::Placed => {
                            setup
                                .audio_man
                                .play(resource_man.audio["tile_placement"].clone())
                                .unwrap();
                        }
                        PlaceTileResponse::Removed => {
                            setup
                                .audio_man
                                .play(resource_man.audio["tile_removal"].clone())
                                .unwrap();
                        }
                        _ => {}
                    }

                    loop_store.already_placed_at = Some(setup.camera.pointing_at)
                }
            }
        }

        if !setup.input_handler.control_held && setup.input_handler.alternate_pressed {
            if let Some(linking_tile) = loop_store.linking_tile {
                on_link_tile(setup, linking_tile);
            } else if loop_store.config_open == Some(setup.camera.pointing_at) {
                loop_store.config_open = None;
                loop_store
                    .gui_state
                    .text_field
                    .get(TextField::Filter)
                    .clear();
            } else {
                loop_store.config_open = Some(setup.camera.pointing_at);
            }
        }

        if setup.input_handler.control_held && loop_store.gui_state.screen == Screen::Ingame {
            if let Some(start) = loop_store.initial_cursor_position {
                if setup.input_handler.tertiary_pressed {
                    let direction = setup.camera.pointing_at - start;

                    setup.game.send_message(GameMsg::MoveTiles(
                        loop_store.grouped_tiles.iter().cloned().collect::<Vec<_>>(),
                        direction,
                        true,
                    ))?;

                    let cap = loop_store.grouped_tiles.capacity();
                    for selected in
                        mem::replace(&mut loop_store.grouped_tiles, HashSet::with_capacity(cap))
                    {
                        let dest = selected + direction;

                        loop_store.grouped_tiles.insert(dest);
                    }

                    loop_store.initial_cursor_position = None;
                    setup.audio_man.play(resource_man.audio["click"].clone())?; // TODO click2
                }
            } else if setup.input_handler.tertiary_pressed {
                loop_store.initial_cursor_position = Some(setup.camera.pointing_at);
                setup.audio_man.play(resource_man.audio["click"].clone())?;
            }

            if loop_store.initial_cursor_position.is_none() {
                loop_store.grouped_tiles.insert(setup.camera.pointing_at);
            }
        } else {
            loop_store.grouped_tiles.clear();
            loop_store.initial_cursor_position = None;
        }

        if setup.input_handler.control_held && setup.input_handler.key_active(KeyActions::Undo) {
            setup.game.send_message(GameMsg::Undo)?;
        }

        if setup.input_handler.key_active(KeyActions::Fullscreen) {
            setup.options.graphics.fullscreen = !setup.options.graphics.fullscreen;
            setup.options.synced = false
        }
    }

    Ok(false)
}
