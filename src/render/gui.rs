use std::collections::HashMap;
use std::f32::consts::FRAC_PI_4;
use std::sync::Arc;

use cgmath::{point2, point3, vec3, MetricSpace};
use egui::epaint::Shadow;
use egui::style::{Margin, WidgetVisuals, Widgets};
use egui::FontFamily::{Monospace, Proportional};
use egui::{
    vec2, Align, Align2, Color32, CursorIcon, DragValue, FontData, FontDefinitions, FontId, Frame,
    PaintCallback, Rgba, Rounding, ScrollArea, Sense, Stroke, Style, TextStyle, TopBottomPanel, Ui,
    Visuals, Window,
};
use egui_winit_vulkano::{CallbackFn, Gui, GuiConfig};
use fuse_rust::Fuse;
use futures::channel::mpsc;
use futures_executor::block_on;
use genmesh::{EmitTriangles, Quad};
use hexagon_tiles::traits::HexDirection;
use riker::actors::{ActorRef, ActorSystem, Tell};
use riker_patterns::ask::ask;
use rune::Any;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::SampleCount::Sample4;
use vulkano::pipeline::{Pipeline, PipelineBindPoint};
use winit::event_loop::EventLoop;

use crate::game::run::event::EventLoopStorage;
use crate::game::tile::coord::TileCoord;
use crate::game::tile::coord::TileHex;
use crate::game::tile::entity::{Data, DataMap, TileEntityMsg, TileState};
use crate::game::GameMsg;
use crate::render::camera::{hex_to_normalized, Camera};
use crate::render::data::{GuiUBO, InstanceData, Vertex};
use crate::render::gpu::Gpu;
use crate::render::renderer::Renderer;
use crate::render::{gpu, gui};
use crate::resource::tile::TileType;
use crate::resource::ResourceManager;
use crate::util::cg::{perspective, DPoint2, DPoint3, Matrix4, Num, Vector3};
use crate::util::colors;
use crate::util::id::{id_static, Id, Interner};
use crate::IOSEVKA_FONT;

#[derive(Clone, Copy, Any)]
pub struct GuiIds {
    #[rune(get, copy)]
    pub tile_config: Id,
    #[rune(get, copy)]
    pub tile_info: Id,
    #[rune(get, copy)]
    pub tile_config_script: Id,
    #[rune(get, copy)]
    pub tile_config_storage: Id,
    #[rune(get, copy)]
    pub tile_config_target: Id,
}

impl GuiIds {
    pub fn new(interner: &mut Interner) -> Self {
        Self {
            tile_config: id_static("automancy", "tile_config").to_id(interner),
            tile_info: id_static("automancy", "tile_info").to_id(interner),
            tile_config_script: id_static("automancy", "tile_config_script").to_id(interner),
            tile_config_storage: id_static("automancy", "tile_config_storage").to_id(interner),
            tile_config_target: id_static("automancy", "tile_config_target").to_id(interner),
        }
    }
}

fn init_fonts(gui: &Gui) {
    let mut fonts = FontDefinitions::default();
    let iosevka = "iosevka".to_owned();

    fonts
        .font_data
        .insert(iosevka.clone(), FontData::from_static(IOSEVKA_FONT));

    fonts
        .families
        .get_mut(&Proportional)
        .unwrap()
        .insert(0, iosevka.clone());
    fonts
        .families
        .get_mut(&Monospace)
        .unwrap()
        .insert(0, iosevka);

    gui.context().set_fonts(fonts);
}

fn init_styles(gui: &Gui) {
    gui.context().set_style(Style {
        override_text_style: None,
        override_font_id: None,
        text_styles: [
            (TextStyle::Small, FontId::new(9.0, Proportional)),
            (TextStyle::Body, FontId::new(13.0, Proportional)),
            (TextStyle::Button, FontId::new(13.0, Proportional)),
            (TextStyle::Heading, FontId::new(19.0, Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, Monospace)),
        ]
        .into(),
        wrap: None,
        visuals: Visuals {
            widgets: Widgets {
                noninteractive: WidgetVisuals {
                    bg_fill: Color32::from_gray(170),
                    bg_stroke: Stroke::new(1.0, Color32::from_gray(160)), // separators, indentation lines
                    fg_stroke: Stroke::new(1.0, Color32::from_gray(80)),  // normal text color
                    rounding: Rounding::same(2.0),
                    expansion: 0.0,
                },
                inactive: WidgetVisuals {
                    bg_fill: Color32::from_gray(200), // checkbox background
                    bg_stroke: Default::default(),
                    fg_stroke: Stroke::new(1.0, Color32::from_gray(60)), // button text
                    rounding: Rounding::same(2.0),
                    expansion: 0.0,
                },
                hovered: WidgetVisuals {
                    bg_fill: Color32::from_gray(190),
                    bg_stroke: Stroke::new(1.0, Color32::from_gray(105)), // e.g. hover over window edge or button
                    fg_stroke: Stroke::new(1.5, Color32::BLACK),
                    rounding: Rounding::same(3.0),
                    expansion: 1.0,
                },
                active: WidgetVisuals {
                    bg_fill: Color32::from_gray(180),
                    bg_stroke: Stroke::new(1.0, Color32::BLACK),
                    fg_stroke: Stroke::new(2.0, Color32::BLACK),
                    rounding: Rounding::same(2.0),
                    expansion: 1.0,
                },
                open: WidgetVisuals {
                    bg_fill: Color32::from_gray(210),
                    bg_stroke: Stroke::new(1.0, Color32::from_gray(160)),
                    fg_stroke: Stroke::new(1.0, Color32::BLACK),
                    rounding: Rounding::same(2.0),
                    expansion: 0.0,
                },
            },
            ..Visuals::light()
        },
        ..Default::default()
    });
}

pub fn init_gui(event_loop: &EventLoop<()>, gpu: &Gpu) -> Gui {
    let gui = Gui::new_with_subpass(
        event_loop,
        gpu.surface.clone(),
        gpu.queue.clone(),
        gpu.gui_subpass.clone(),
        GuiConfig {
            preferred_format: Some(gpu.alloc.swapchain.image_format()),
            is_overlay: true,
            samples: Sample4,
        },
    );

    init_fonts(&gui);
    init_styles(&gui);

    gui
}

pub fn default_frame() -> Frame {
    Frame::none()
        .fill(colors::WHITE.multiply(0.65).into())
        .shadow(Shadow {
            extrusion: 8.0,
            color: colors::DARK_GRAY.multiply(0.5).into(),
        })
        .rounding(Rounding::same(5.0))
}

fn tile_paint(
    ui: &mut Ui,
    resource_man: Arc<ResourceManager>,
    gpu: &Gpu,
    size: f32,
    id: Id,
    model: Id,
    selection_send: &mut mpsc::Sender<Id>,
) -> PaintCallback {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());

    response.clone().on_hover_text(resource_man.tile_name(&id));
    response.clone().on_hover_cursor(CursorIcon::Grab);

    let hover = if response.hovered() {
        ui.ctx()
            .animate_value_with_time(ui.next_auto_id(), 1.0, 0.3)
    } else {
        ui.ctx()
            .animate_value_with_time(ui.next_auto_id(), 0.0, 0.3)
    };
    if response.clicked() {
        selection_send.try_send(id).unwrap();
    }

    let pos = point3(0.0, 0.0, 1.0 - (0.5 * hover));
    let eye = point3(pos.x, pos.y, pos.z - 0.3);
    let matrix = perspective(FRAC_PI_4, 1.0, 0.01, 10.0)
        * Matrix4::from_translation(vec3(0.0, 0.0, 2.0))
        * Matrix4::look_to_rh(eye, vec3(0.0, 1.0 - pos.z, 1.0), Vector3::unit_y());

    let pipeline = gpu.gui_pipeline.clone();
    let vertex_buffer = gpu.alloc.vertex_buffer.clone();
    let index_buffer = gpu.alloc.index_buffer.clone();
    let ubo_layout = pipeline.layout().set_layouts()[0].clone();

    PaintCallback {
        rect,
        callback: Arc::new(CallbackFn::new(move |_info, context| {
            let uniform_buffer = gpu::uniform_buffer(&context.resources.memory_allocator);

            let ubo = GuiUBO {
                matrix: matrix.into(),
            };

            *uniform_buffer.write().unwrap() = ubo;

            let ubo_set = PersistentDescriptorSet::new(
                context.resources.descriptor_set_allocator,
                ubo_layout.clone(),
                [WriteDescriptorSet::buffer(0, uniform_buffer)],
            )
            .unwrap();

            let instance = InstanceData::new().model(model);

            if let Some((indirect_commands, instance_buffer)) = gpu::indirect_instance(
                &context.resources.memory_allocator,
                &resource_man,
                &[instance],
            ) {
                context
                    .builder
                    .bind_pipeline_graphics(pipeline.clone())
                    .bind_vertex_buffers(0, (vertex_buffer.clone(), instance_buffer))
                    .bind_index_buffer(index_buffer.clone())
                    .bind_descriptor_sets(
                        PipelineBindPoint::Graphics,
                        pipeline.layout().clone(),
                        0,
                        ubo_set,
                    )
                    .draw_indexed_indirect(indirect_commands)
                    .unwrap();
            }
        })),
    }
}

fn paint_tile_selection(
    ui: &mut Ui,
    resource_man: Arc<ResourceManager>,
    selected_tile_states: &HashMap<Id, TileState>,
    gpu: &Gpu,
    mut selection_send: mpsc::Sender<Id>,
) {
    let size = ui.available_height();

    resource_man
        .ordered_tiles
        .iter()
        .flat_map(|id| {
            let resource = &resource_man.registry.get_tile(*id).unwrap();

            if resource.tile_type == TileType::Model {
                return None;
            }

            resource
                .models
                .get(*selected_tile_states.get(id).unwrap_or(&0) as usize)
                .map(|v| (*id, *v))
        })
        .for_each(|(id, faces_index)| {
            let callback = tile_paint(
                ui,
                resource_man.clone(),
                gpu,
                size,
                id,
                faces_index,
                &mut selection_send,
            );

            ui.painter().add(callback);
        });
}

pub fn tile_selections(
    gui: &mut Gui,
    resource_man: Arc<ResourceManager>,
    selected_tile_states: &HashMap<Id, TileState>,
    renderer: &Renderer,
    selection_send: mpsc::Sender<Id>,
) {
    TopBottomPanel::bottom("tile_selections")
        .show_separator_line(false)
        .resizable(false)
        .frame(default_frame().outer_margin(Margin::same(10.0)))
        .show(&gui.context(), |ui| {
            let spacing = ui.spacing_mut();

            spacing.interact_size.y = 70.0;
            spacing.scroll_bar_width = 0.0;
            spacing.scroll_bar_outer_margin = 0.0;

            ScrollArea::horizontal()
                .always_show_scroll(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        paint_tile_selection(
                            ui,
                            resource_man.clone(),
                            selected_tile_states,
                            &renderer.gpu,
                            selection_send,
                        );
                    });
                });
        });
}

pub fn tile_info(
    gui: &mut Gui,
    resource_man: Arc<ResourceManager>,
    sys: &ActorSystem,
    game: ActorRef<GameMsg>,
    pointing_at: TileCoord,
) {
    Window::new(resource_man.translates.gui[&resource_man.registry.gui_ids.tile_info].to_string())
        .anchor(Align2([Align::RIGHT, Align::TOP]), vec2(-10.0, 10.0))
        .resizable(false)
        .default_width(300.0)
        .frame(default_frame().inner_margin(Margin::same(10.0)))
        .show(&gui.context(), |ui| {
            ui.colored_label(colors::DARK_GRAY, pointing_at.to_string());

            let result: Option<(ActorRef<TileEntityMsg>, Id, TileState)> =
                block_on(ask(sys, &game, GameMsg::GetTile(pointing_at)));

            if let Some((tile, id, _)) = result {
                ui.label(resource_man.tile_name(&id));
                let data: DataMap = block_on(ask(sys, &tile, TileEntityMsg::GetData));

                if let Some(inventory) = data.get("buffer").and_then(Data::as_inventory) {
                    for (id, amount) in inventory.0.iter() {
                        ui.label(format!("{} - {}", resource_man.item_name(id), amount));
                    }
                }
                //ui.label(format!("State: {}", ask(sys, &game, )))
            }
        });
}

pub fn add_direction(ui: &mut Ui, target_coord: &mut Option<TileCoord>, n: usize) {
    let coord = TileHex::NEIGHBORS[(n + 2) % 6];
    let coord = Some(coord.into());

    ui.selectable_value(
        target_coord,
        coord,
        match n {
            0 => "↗",
            1 => "➡",
            2 => "↘",
            3 => "↙",
            4 => "⬅",
            5 => "↖",
            _ => "",
        },
    );
}

pub fn searchable_id(
    ui: &mut Ui,
    resource_man: Arc<ResourceManager>,
    fuse: &Fuse,
    ids: &[Id],
    new_id: &mut Option<Id>,
    filter: &mut String,
) {
    ui.text_edit_singleline(filter);

    ScrollArea::vertical().max_height(80.0).show(ui, |ui| {
        ui.set_width(ui.available_width());

        let ids = if !filter.is_empty() {
            let mut filtered = ids
                .iter()
                .flat_map(|id| {
                    let result = fuse.search_text_in_string(filter, resource_man.item_name(id));
                    let score = result.map(|v| v.score);

                    if score.unwrap_or(0.0) > 0.4 {
                        None
                    } else {
                        Some(*id).zip(score)
                    }
                })
                .collect::<Vec<_>>();

            filtered.sort_unstable_by(|a, b| a.1.total_cmp(&b.1));

            filtered.into_iter().map(|v| v.0).collect::<Vec<_>>()
        } else {
            ids.to_vec()
        };

        ids.iter().for_each(|script| {
            ui.radio_value(new_id, Some(*script), resource_man.item_name(script));
        })
    });
}

pub fn targets(ui: &mut Ui, new_target_coord: &mut Option<TileCoord>) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.add_space(15.0);
            add_direction(ui, new_target_coord, 5);
            add_direction(ui, new_target_coord, 0);
        });

        ui.horizontal(|ui| {
            add_direction(ui, new_target_coord, 4);
            ui.selectable_value(new_target_coord, None, "❌");
            add_direction(ui, new_target_coord, 1);
        });

        ui.horizontal(|ui| {
            ui.add_space(15.0);
            add_direction(ui, new_target_coord, 3);
            add_direction(ui, new_target_coord, 2);
        });
    });
}

pub fn tile_config(
    gui: &mut Gui,
    resource_man: Arc<ResourceManager>,
    loop_store: &mut EventLoopStorage,
    extra_vertices: &mut Vec<Vertex>,
    camera: &Camera,
    sys: &ActorSystem,
    game: ActorRef<GameMsg>,
    frame: Frame,
) {
    if let Some(config_open) = loop_store.config_open {
        let result: Option<(ActorRef<TileEntityMsg>, Id, TileState)> =
            block_on(ask(sys, &game, GameMsg::GetTile(config_open)));

        if let Some((tile, id, _c)) = result {
            let data: DataMap = block_on(ask(sys, &tile, TileEntityMsg::GetData));

            let current_amount = data
                .get("amount")
                .and_then(Data::as_amount)
                .cloned()
                .unwrap_or(0);
            let mut new_amount = current_amount;

            let current_script = data.get("script").and_then(Data::as_id).cloned();
            let mut new_script = current_script;

            let current_storage = data.get("storage").and_then(Data::as_id).cloned();
            let mut new_storage = current_storage;

            let current_target_coord = data.get("target").and_then(Data::as_coord).cloned();
            let mut new_target_coord = current_target_coord;

            // tile_config
            Window::new(
                resource_man.translates.gui[&resource_man.registry.gui_ids.tile_config].to_string(),
            )
            .resizable(false)
            .auto_sized()
            .constrain(true)
            .frame(frame.inner_margin(Margin::same(10.0)))
            .show(&gui.context(), |ui| {
                const MARGIN: Num = 8.0;

                ui.set_max_width(300.0);

                match &resource_man.registry.get_tile(id).unwrap().tile_type {
                    TileType::Machine(scripts) => {
                        let script_text = if let Some(script) =
                            new_script.and_then(|id| resource_man.registry.get_script(id))
                        {
                            let input = if let Some(inputs) = script.instructions.inputs {
                                inputs
                                    .iter()
                                    .map(|item_stack| {
                                        format!(
                                            "{} ({})",
                                            resource_man.item_name(&item_stack.item.id),
                                            item_stack.amount
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            } else {
                                String::new()
                            };

                            let output = if let Some(output) = script.instructions.output {
                                format!(
                                    "=> {} ({})",
                                    resource_man.item_name(&output.item.id),
                                    output.amount
                                )
                            } else {
                                String::new()
                            };

                            if !input.is_empty() && !output.is_empty() {
                                format!("{input}\n{output}")
                            } else {
                                format!("{input}{output}")
                            }
                        } else {
                            "<none>".to_string()
                        };

                        ui.add_space(MARGIN);

                        ui.label(
                            resource_man.translates.gui
                                [&resource_man.registry.gui_ids.tile_config_script]
                                .as_str(),
                        );
                        ui.label(script_text);

                        ui.add_space(MARGIN);

                        searchable_id(
                            ui,
                            resource_man.clone(),
                            &loop_store.fuse,
                            scripts.as_slice(),
                            &mut new_script,
                            &mut loop_store.filter,
                        );
                    }
                    TileType::Storage(storage) => {
                        let storage_text = if let Some(item) =
                            new_storage.and_then(|id| resource_man.registry.get_item(id))
                        {
                            resource_man.item_name(&item.id).to_string()
                        } else {
                            "<none>".to_string()
                        };

                        let items = resource_man
                            .get_items(storage.id, &mut loop_store.tag_cache)
                            .iter()
                            .map(|item| item.id)
                            .collect::<Vec<_>>();

                        ui.add_space(MARGIN);

                        ui.label(
                            resource_man.translates.gui
                                [&resource_man.registry.gui_ids.tile_config_storage]
                                .as_str(),
                        );
                        ui.horizontal(|ui| {
                            ui.label(storage_text);
                            ui.add(
                                DragValue::new(&mut new_amount)
                                    .clamp_range(0..=65535)
                                    .speed(1.0)
                                    .prefix("Amount:"), // TODO translate
                            );
                        });

                        ui.add_space(MARGIN);

                        searchable_id(
                            ui,
                            resource_man.clone(),
                            &loop_store.fuse,
                            items.as_slice(),
                            &mut new_storage,
                            &mut loop_store.filter,
                        );
                    }
                    TileType::Transfer(id) => {
                        if id == &resource_man.registry.tile_ids.inventory_linker {
                            ui.add_space(MARGIN);

                            if ui.button("Link Network!").clicked() {
                                loop_store.linking_tile = Some(config_open);
                            };
                            ui.label("(Right click to link Destination)");

                            ui.add_space(MARGIN);
                        }

                        if id == &resource_man.registry.tile_ids.inventory_provider {
                            let result: Option<Data> = block_on(ask(
                                sys,
                                &game,
                                GameMsg::SendMsgToTile(
                                    config_open,
                                    TileEntityMsg::GetDataValue("link".to_string()),
                                ),
                            ));

                            if let Some(link) = result.as_ref().and_then(Data::as_coord) {
                                let DPoint3 { x, y, .. } = hex_to_normalized(
                                    camera.window_size.0,
                                    camera.window_size.1,
                                    camera.camera_state().pos,
                                    config_open,
                                );
                                let a = point2(x, y);

                                let DPoint3 { x, y, .. } = hex_to_normalized(
                                    camera.window_size.0,
                                    camera.window_size.1,
                                    camera.camera_state().pos,
                                    config_open + *link,
                                );
                                let b = point2(x, y);

                                extra_vertices.append(&mut gui::line(a, b, colors::RED));
                            }
                        }
                    }
                    _ => {}
                }

                if resource_man.registry.get_tile(id).unwrap().targeted {
                    ui.add_space(MARGIN);

                    ui.label(
                        resource_man.translates.gui
                            [&resource_man.registry.gui_ids.tile_config_target]
                            .as_str(),
                    );
                    targets(ui, &mut new_target_coord);
                }

                ui.add_space(MARGIN);
            });

            if new_amount != current_amount {
                tile.tell(
                    TileEntityMsg::SetData("amount".to_string(), Data::Amount(new_amount)),
                    None,
                );
            }

            if new_script != current_script {
                if let Some(script) = new_script {
                    tile.tell(
                        TileEntityMsg::SetData("script".to_string(), Data::Id(script)),
                        None,
                    );
                    tile.tell(TileEntityMsg::RemoveData("buffer".to_string()), None);
                }
            }

            if new_storage != current_storage {
                if let Some(storage) = new_storage {
                    tile.tell(
                        TileEntityMsg::SetData("storage".to_string(), Data::Id(storage)),
                        None,
                    );
                    tile.tell(TileEntityMsg::RemoveData("buffer".to_string()), None);
                }
            }

            if new_target_coord != current_target_coord {
                if let Some(target_coord) = new_target_coord {
                    game.send_msg(
                        GameMsg::SendMsgToTile(
                            config_open,
                            TileEntityMsg::SetData("target".to_string(), Data::Coord(target_coord)),
                        ),
                        None,
                    );
                } else {
                    game.send_msg(
                        GameMsg::SendMsgToTile(
                            config_open,
                            TileEntityMsg::RemoveData("target".to_string()),
                        ),
                        None,
                    );
                }
            }
        }
    }
}

pub fn line(a: DPoint2, b: DPoint2, color: Rgba) -> Vec<Vertex> {
    let v = b - a;
    let l = a.distance(b) * 128.0;
    let w = cgmath::vec2(-v.y / l, v.x / l);

    let a0 = (a + w).cast::<Num>().unwrap();
    let a1 = (b + w).cast::<Num>().unwrap();
    let b0 = (b - w).cast::<Num>().unwrap();
    let b1 = (a - w).cast::<Num>().unwrap();

    let mut line = vec![];

    Quad::new(
        Vertex {
            pos: [a0.x, a0.y, 0.0],
            color: color.to_array(),
            normal: [0.0, 0.0, 0.0],
        },
        Vertex {
            pos: [a1.x, a1.y, 0.0],
            color: color.to_array(),
            normal: [0.0, 0.0, 0.0],
        },
        Vertex {
            pos: [b0.x, b0.y, 0.0],
            color: color.to_array(),
            normal: [0.0, 0.0, 0.0],
        },
        Vertex {
            pos: [b1.x, b1.y, 0.0],
            color: color.to_array(),
            normal: [0.0, 0.0, 0.0],
        },
    )
    .emit_triangles(|v| line.append(&mut vec![v.x, v.y, v.z]));

    line
}
