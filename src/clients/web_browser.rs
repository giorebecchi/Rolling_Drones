use wg_2024::packet;
use wg_2024::controller;
use serde::{Serialize, Deserialize};

pub enum CommandWebBrowser {
    TextList, //to retrieve text file list
    TextFile { //to send command for one single file
        file_name: String,
        media: bool //to see if it also needed to retrieve the referenced media
    }
}


