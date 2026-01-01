use wasm_bindgen::prelude::*;
use js_sys::{Array, Reflect};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileKind {
    File,
    Directory,
}

#[derive(Clone)]
pub(crate) struct FileEntry {
    pub(crate) path: String,
    #[allow(dead_code)]
    pub(crate) name: String,
    #[allow(dead_code)]
    pub(crate) depth: usize,
    pub(crate) kind: FileKind,
    pub(crate) handle: JsValue,
}

#[wasm_bindgen(inline_js = "
export async function openDirectoryPicker() {
  if (!('showDirectoryPicker' in window)) {
    throw new Error('File system access not supported');
  }
  return await window.showDirectoryPicker();
}

async function walkDirectory(handle, prefix) {
  const entries = [];
  for await (const entry of handle.values()) {
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.kind === 'directory') {
      entries.push({ path, kind: 'directory', handle: entry });
      const nested = await walkDirectory(entry, path);
      for (const item of nested) {
        entries.push(item);
      }
    } else {
      entries.push({ path, kind: 'file', handle: entry });
    }
  }
  return entries;
}

export async function listDirectoryEntries(handle) {
  return await walkDirectory(handle, '');
}

export async function readFileHandle(handle) {
  const file = await handle.getFile();
  return await file.text();
}

export async function writeFileHandle(handle, contents) {
  const writable = await handle.createWritable();
  await writable.write(contents);
  await writable.close();
}
")]
extern "C" {
    #[wasm_bindgen(js_name = openDirectoryPicker, catch)]
    async fn open_directory_picker() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = listDirectoryEntries, catch)]
    async fn list_directory_entries(handle: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = readFileHandle, catch)]
    async fn read_file_handle(handle: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = writeFileHandle, catch)]
    async fn write_file_handle(handle: JsValue, contents: String) -> Result<(), JsValue>;
}

pub(crate) async fn pick_directory_entries() -> Result<Vec<FileEntry>, String> {
    let handle = open_directory_picker().await.map_err(js_error)?;
    let entries = list_directory_entries(handle).await.map_err(js_error)?;
    let array = Array::from(&entries);

    let mut result = Vec::new();
    for entry in array.iter() {
        let path = Reflect::get(&entry, &JsValue::from_str("path"))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let kind_str = Reflect::get(&entry, &JsValue::from_str("kind"))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let kind = if kind_str == "directory" {
            FileKind::Directory
        } else {
            FileKind::File
        };
        let handle = Reflect::get(&entry, &JsValue::from_str("handle"))
            .unwrap_or(JsValue::NULL);
        let name = path.split('/').last().unwrap_or(&path).to_string();
        let depth = if path.is_empty() {
            0
        } else {
            path.matches('/').count()
        };

        result.push(FileEntry {
            path,
            name,
            depth,
            kind,
            handle,
        });
    }

    result.sort_by(|a, b| {
        match (a.kind, b.kind) {
            (FileKind::Directory, FileKind::File) => std::cmp::Ordering::Less,
            (FileKind::File, FileKind::Directory) => std::cmp::Ordering::Greater,
            _ => a.path.cmp(&b.path),
        }
    });
    Ok(result)
}

pub(crate) async fn read_file(handle: &JsValue) -> Result<String, String> {
    let text = read_file_handle(handle.clone()).await.map_err(js_error)?;
    text.as_string().ok_or_else(|| "Failed to decode file text".to_string())
}

pub(crate) async fn write_file(handle: &JsValue, contents: &str) -> Result<(), String> {
    write_file_handle(handle.clone(), contents.to_string())
        .await
        .map_err(js_error)
}

fn js_error(error: JsValue) -> String {
    if let Some(message) = error.as_string() {
        return message;
    }
    if let Ok(value) = Reflect::get(&error, &JsValue::from_str("message")) {
        if let Some(message) = value.as_string() {
            return message;
        }
    }
    "Unknown error".to_string()
}
