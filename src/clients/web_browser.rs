use wg_2024::packet;
use wg_2024::controller;
use serde::{Serialize, Deserialize};
use crate::clients::assembler::MessageWeb;

pub enum CommandWebBrowser {
    ServerType,
    TextList, //to retrieve text file list
    TextFile (MessageWeb)
}


