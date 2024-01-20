use std::f64::consts::FRAC_PI_4;

use automancy_defs::glam::{dvec3, vec3};
use egui::scroll_area::ScrollBarVisibility;
use egui::{vec2, Context, CursorIcon, Margin, ScrollArea, Sense, TopBottomPanel, Ui};
use futures::channel::mpsc;

use automancy_defs::id::Id;
use automancy_defs::math;
use automancy_defs::math::{z_far, z_near, Matrix4};
use automancy_defs::rendering::InstanceData;
use automancy_resources::data::{Data, DataMap};

use crate::gui::{default_frame, GameEguiCallback};
use crate::setup::GameSetup;

/// Draws the tile selection.
fn draw_tile_selection(
    setup: &GameSetup,
    ui: &mut Ui,
    mut selection_send: mpsc::Sender<Id>,
    game_data: &DataMap,
) {
    let size = ui.available_height();
    let projection =
        math::perspective(FRAC_PI_4, 1.0, z_near(), z_far()) * math::view(dvec3(0.0, 0.0, 2.75));
    let projection = projection.as_mat4();

    for id in setup.resource_man.ordered_tiles.iter().filter(|id| {
        if setup
            .resource_man
            .registry
            .tiles
            .get(*id)
            .unwrap()
            .data
            .get(&setup.resource_man.registry.data_ids.default_tile)
            .cloned()
            .and_then(Data::into_bool)
            .unwrap_or(false)
        {
            return true;
        }

        if let Some(research) = setup.resource_man.get_research_by_unlock(**id) {
            if let Some(Data::SetId(unlocked)) =
                game_data.get(&setup.resource_man.registry.data_ids.unlocked_researches)
            {
                return unlocked.contains(&research.id);
            }
        }

        false
    }) {
        let tile = setup.resource_man.registry.tiles.get(id).unwrap();
        let model = setup.resource_man.get_model(tile.model);

        let (ui_id, rect) = ui.allocate_space(vec2(size, size));

        let response = ui.interact(rect, ui_id, Sense::click());

        response
            .clone()
            .on_hover_text(setup.resource_man.tile_name(id));
        response.clone().on_hover_cursor(CursorIcon::Grab);

        let hover = if response.hovered() {
            ui.ctx()
                .animate_value_with_time(ui.next_auto_id(), 0.75, 0.3)
        } else {
            ui.ctx()
                .animate_value_with_time(ui.next_auto_id(), 0.25, 0.3)
        };
        if response.clicked() {
            selection_send.try_send(*id).unwrap();
        }

        let rotate = Matrix4::from_rotation_x(hover);

        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            GameEguiCallback::new(
                InstanceData::default()
                    .with_model_matrix(rotate)
                    .with_projection(projection)
                    .with_light_pos(vec3(0.0, 4.0, 14.0), None),
                model,
                rect,
                ui.ctx().screen_rect(),
            ),
        ));
    }
}

/// Creates the tile selection GUI.
pub fn tile_selections(
    setup: &GameSetup,
    context: &Context,
    selection_send: mpsc::Sender<Id>,
    game_data: &DataMap,
) {
    TopBottomPanel::bottom("tile_selections")
        .show_separator_line(false)
        .resizable(false)
        .frame(default_frame().outer_margin(Margin::same(10.0)))
        .show(context, |ui| {
            ScrollArea::horizontal()
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_height(90.0);

                        draw_tile_selection(setup, ui, selection_send, game_data);
                    });
                });
        });
}
