use std::{fs, path::Path};
use walkdir::WalkDir;
use zvariant::{Structure, Value};

pub fn find_icon(icon_name: &str) -> Option<String> {
    // Check whether the icon needs to be searched
    if icon_name.starts_with('/') {
        return Some(icon_name.to_string());
    } else if icon_name.starts_with('~') {
        return Some(
            icon_name.replace('~', format!("{}/", std::env::var("HOME").unwrap()).as_str()),
        );
    }
    let icon_dir = "/usr/share/icons/AdwaitaLegacy/48x48/";
    // Recursively search for the icon in the icon directories
    for entry in WalkDir::new(icon_dir) {
        if entry.is_err() {
            continue;
        }
        let entry = entry.unwrap();
        if entry.file_name().to_string_lossy().contains(icon_name) {
            return Some(entry.path().to_string_lossy().to_string());
        }
    }
    None
}

pub fn save_icon(icon_data: &Structure, id: u32) -> Option<String> {
    let parent_dir = "/tmp/end-data";
    if !Path::new(&parent_dir).exists() {
        fs::create_dir_all(parent_dir).unwrap();
    }
    let icon_path = format!("{}/{}.png", parent_dir, id);
    let width: Result<i32, zvariant::Error> = icon_data.fields()[0].try_clone().unwrap().try_into();
    let height: Result<i32, zvariant::Error> =
        icon_data.fields()[1].try_clone().unwrap().try_into();
    let _rowstride: Result<i32, zvariant::Error> =
        icon_data.fields()[2].try_clone().unwrap().try_into();
    let _has_alpha: Result<bool, zvariant::Error> =
        icon_data.fields()[3].try_clone().unwrap().try_into();
    let _bits_per_sample: Result<i32, zvariant::Error> =
        icon_data.fields()[4].try_clone().unwrap().try_into();
    let _channels: Result<i32, zvariant::Error> =
        icon_data.fields()[5].try_clone().unwrap().try_into();
    let data = icon_data.fields()[6].try_clone();
    let mut vec_val: Vec<u8> = Vec::new();
    if width.is_err() || height.is_err() || data.is_err() {
        return None;
    }
    let width = width.unwrap();
    let height = height.unwrap();
    let data = data.unwrap();
    match data {
        Value::Array(data) => {
            data.iter()
                .map(|x| vec_val.push(x.try_clone().unwrap().try_into().unwrap()))
                .count();
        }
        _ => {
            return None;
        }
    }
    let res = image::save_buffer(
        &icon_path,
        &vec_val,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    );
    if res.is_err() {
        return None;
    }
    Some(icon_path)
}
