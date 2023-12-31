use egui::scroll_area::ScrollBarVisibility;
use egui::{vec2, Context, CursorIcon, Margin, ScrollArea, Sense, TopBottomPanel, Ui};
use futures::channel::mpsc;
use std::f32::consts::FRAC_PI_4;

use automancy_defs::cgmath::{point3, Rotation3};
use automancy_defs::id::Id;
use automancy_defs::math;
use automancy_defs::math::{rad, z_far, z_near, Matrix4, Quaternion};
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
        math::perspective(FRAC_PI_4, 1.0, z_near(), z_far()) * math::view(point3(0.0, 0.0, 2.75));

    for id in setup.resource_man.ordered_tiles.iter().filter(|id| {
        if Some(&true)
            == setup
                .resource_man
                .registry
                .tile(**id)
                .unwrap()
                .data
                .get(&setup.resource_man.registry.data_ids.default_tile)
                .and_then(Data::as_bool)
        {
            return true;
        }

        if let Some(research) = setup.resource_man.get_research_by_unlock(**id) {
            if let Some(unlocked) = game_data
                .get(&setup.resource_man.registry.data_ids.unlocked_researches)
                .and_then(Data::as_set_id)
            {
                return unlocked.contains(&research.id);
            }
        }

        false
    }) {
        let tile = setup.resource_man.registry.tile(*id).unwrap();
        let model = setup.resource_man.get_model(tile.model);

        let (ui_id, rect) = ui.allocate_space(vec2(size, size));
        if !ui.ctx().screen_rect().contains_rect(rect) {
            continue;
        }

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

        let rotate = Quaternion::from_angle_x(rad(hover));

        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            GameEguiCallback::new(
                InstanceData::default()
                    .with_model_matrix(Matrix4::from(rotate))
                    .with_projection(projection)
                    .with_light_pos(point3(0.0, 4.0, 14.0), None),
                model,
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
                        ui.set_height(80.0);

                        draw_tile_selection(setup, ui, selection_send, game_data);
                    });
                });
        });
}
