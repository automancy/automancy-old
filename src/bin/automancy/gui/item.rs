use automancy_defs::cgmath::point3;
use automancy_defs::math;
use egui::{vec2, Rect, Response, Sense, Ui};

use crate::gui::GameEguiCallback;
use automancy_defs::math::Float;
use automancy_defs::rendering::InstanceData;
use automancy_resources::data::stack::ItemStack;
use automancy_resources::ResourceManager;

pub const SMALL_ITEM_ICON_SIZE: Float = 24.0;
pub const MEDIUM_ITEM_ICON_SIZE: Float = 48.0;
pub const LARGE_ITEM_ICON_SIZE: Float = 96.0;

/// Draws an Item's icon.
pub fn draw_item(
    ui: &mut Ui,
    resource_man: &ResourceManager,
    prefix: Option<&'static str>,
    stack: ItemStack,
    size: Float,
    add_label: bool,
) -> (Rect, Response) {
    ui.horizontal(|ui| {
        ui.set_height(size);

        ui.style_mut().spacing.item_spacing = vec2(10.0, 0.0);

        if let Some(prefix) = prefix {
            ui.label(prefix);
        }

        let (rect, icon_response) = ui.allocate_exact_size(vec2(size, size), Sense::click());

        let response = if add_label {
            let label_response = if stack.amount > 0 {
                ui.label(format!(
                    "{} ({})",
                    resource_man.item_name(&stack.item.id),
                    stack.amount
                ))
            } else {
                ui.label(resource_man.item_name(&stack.item.id).to_string())
            };

            icon_response.union(label_response)
        } else {
            icon_response
        };

        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            GameEguiCallback::new(
                InstanceData::default().with_projection(math::view(point3(0.0, 0.0, 1.0))),
                resource_man.get_item_model(stack.item),
                rect,
                ui.ctx().screen_rect(),
            ),
        ));

        (rect, response)
    })
    .inner
}
