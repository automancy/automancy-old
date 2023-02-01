use serde::Deserialize;

use crate::game::item::{Item, ItemRaw};
use crate::util::id::{Id, IdRaw};

#[derive(Debug, Clone, Copy)]
pub struct Script {
    pub id: Id,
    pub instructions: Instructions,
}

#[derive(Debug, Clone, Copy)]
pub struct Instructions {
    pub input: Option<Item>,
    pub output: Option<Item>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScriptRaw {
    pub id: IdRaw,
    pub instructions: InstructionsRaw,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstructionsRaw {
    pub input: Option<ItemRaw>,
    pub output: Option<ItemRaw>,
}
