use egui::{vec2, Align2, Context, Window};
use futures::executor::block_on;

use automancy_defs::colors;
use automancy_resources::data::stack::ItemStack;
use automancy_resources::data::Data;

use crate::game::GameMsg;
use crate::gui::item::draw_item;
use crate::gui::{default_frame, SMALL_ICON_SIZE};
use crate::setup::GameSetup;
use crate::tile_entity::TileEntityMsg;

/// Draws the info GUI.
pub fn info(setup: &GameSetup, context: &Context) {
    Window::new(
        setup.resource_man.translates.gui[&setup.resource_man.registry.gui_ids.info].as_str(),
    )
    .anchor(Align2::RIGHT_TOP, vec2(-10.0, 10.0))
    .resizable(false)
    .default_width(300.0)
    .frame(default_frame())
    .show(context, |ui| {
        ui.colored_label(colors::DARK_GRAY, setup.camera.pointing_at.to_string());

        let Some(id) = block_on(setup.game.call(
            |reply| GameMsg::GetTile(setup.camera.pointing_at, reply),
            None,
        ))
        .unwrap()
        .unwrap() else {
            return;
        };

        ui.label(setup.resource_man.tile_name(&id));

        let Some(entity) = block_on(setup.game.call(
            |reply| GameMsg::GetTileEntity(setup.camera.pointing_at, reply),
            None,
        ))
        .unwrap()
        .unwrap() else {
            return;
        };

        let data = block_on(entity.call(TileEntityMsg::GetData, None))
            .unwrap()
            .unwrap();

        if let Some(Data::Inventory(inventory)) =
            data.get(&setup.resource_man.registry.data_ids.buffer)
        {
            for (id, amount) in inventory.iter() {
                let item = setup.resource_man.registry.items.get(id).unwrap();

                draw_item(
                    ui,
                    &setup.resource_man,
                    None,
                    ItemStack {
                        item: *item,
                        amount: *amount,
                    },
                    SMALL_ICON_SIZE,
                    true,
                );
            }
        }
    });
}
