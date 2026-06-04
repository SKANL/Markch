use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST_FILE: &str = ".markch-document.json";
const DOCUMENT_TYPE: &str = "markch-document";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPageManifest {
    pub file: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentManifest {
    #[serde(rename = "type")]
    pub document_type: String,
    pub version: u32,
    pub title: String,
    pub pages: Vec<DocumentPageManifest>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMetadata {
    pub path: String,
    pub title: String,
    pub page_count: usize,
    pub modified: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPage {
    pub id: String,
    pub file: String,
    pub title: String,
    pub modified: i64,
    pub index: usize,
    pub word_count: usize,
    pub overflow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDetail {
    pub path: String,
    pub title: String,
    pub pages: Vec<DocumentPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentNormalizeResult {
    pub document: DocumentDetail,
    pub changed: bool,
    pub target_note_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Up,
    Down,
}

#[derive(Debug, Clone)]
enum BlockKind {
    Heading,
    Paragraph,
    Atomic,
}

#[derive(Debug, Clone)]
struct MarkdownBlock {
    text: String,
    words: usize,
    kind: BlockKind,
}

#[derive(Debug, Clone)]
struct PageDraft {
    title: String,
    content: String,
}

struct PaginatedWriteResult {
    document: DocumentDetail,
    changed: bool,
}

fn now_string() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn modified_seconds(path: &Path) -> i64 {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn normalize_rel_path(path: &str) -> String {
    path.trim_matches('/').replace('\\', "/")
}

fn validate_document_path(path: &str) -> Result<(), String> {
    let normalized = normalize_rel_path(path);
    if normalized.is_empty() {
        return Err("Document path cannot be empty".to_string());
    }
    if normalized.contains('\0') {
        return Err("Invalid document path".to_string());
    }
    for part in normalized.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err("Invalid document path".to_string());
        }
    }
    Ok(())
}

fn validate_page_file(file: &str) -> Result<(), String> {
    if file.contains('/') || file.contains('\\') || file.contains('\0') {
        return Err("Invalid page file".to_string());
    }
    if !file.ends_with(".md") || file == ".md" {
        return Err("Page file must be markdown".to_string());
    }
    Ok(())
}

fn document_dir(notes_root: &Path, path: &str) -> Result<PathBuf, String> {
    validate_document_path(path)?;
    let normalized = normalize_rel_path(path);
    let dir = notes_root.join(normalized.replace('/', std::path::MAIN_SEPARATOR_STR));
    if !dir.starts_with(notes_root) {
        return Err("Invalid document path: escapes notes folder".to_string());
    }
    Ok(dir)
}

fn manifest_path(document_dir: &Path) -> PathBuf {
    document_dir.join(MANIFEST_FILE)
}

fn read_manifest(document_dir: &Path) -> Result<DocumentManifest, String> {
    let path = manifest_path(document_dir);
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let manifest: DocumentManifest = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    if manifest.document_type != DOCUMENT_TYPE || manifest.version != 1 {
        return Err("Unsupported Markch document manifest".to_string());
    }
    Ok(manifest)
}

fn write_manifest(document_dir: &Path, manifest: &DocumentManifest) -> Result<(), String> {
    let path = manifest_path(document_dir);
    let content = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

fn page_id(document_path: &str, file: &str) -> String {
    let stem = file.strip_suffix(".md").unwrap_or(file);
    format!("{}/{}", normalize_rel_path(document_path), stem)
}

fn file_from_note_id(note_id: &str) -> Option<(String, String)> {
    let pos = note_id.rfind('/')?;
    let document_path = note_id[..pos].to_string();
    let file = format!("{}.md", &note_id[pos + 1..]);
    Some((document_path, file))
}

fn page_file_name(index: usize, title: &str) -> String {
    let sanitized = crate::sanitize_filename(title);
    format!("{:03}-{}.md", index, sanitized)
}

fn page_index_for_file(manifest: &DocumentManifest, file: &str) -> usize {
    if let Some(index) = manifest.pages.iter().position(|page| page.file == file) {
        return index;
    }
    file.split_once('-')
        .and_then(|(prefix, _)| prefix.parse::<usize>().ok())
        .and_then(|index| index.checked_sub(1))
        .unwrap_or(0)
}

fn target_note_id_for_index(document: &DocumentDetail, index: usize) -> Option<String> {
    document
        .pages
        .get(index)
        .or_else(|| document.pages.last())
        .map(|page| page.id.clone())
}

fn ensure_unique_folder(
    notes_root: &Path,
    parent: Option<&str>,
    title: &str,
) -> Result<String, String> {
    let base_name = crate::sanitize_filename(title);
    let parent = parent.map(normalize_rel_path).filter(|p| !p.is_empty());
    if let Some(ref parent_path) = parent {
        validate_document_path(parent_path)?;
    }

    let mut candidate_name = base_name.clone();
    let mut counter = 1;
    loop {
        let rel = if let Some(ref parent_path) = parent {
            format!("{}/{}", parent_path, candidate_name)
        } else {
            candidate_name.clone()
        };
        let dir = document_dir(notes_root, &rel)?;
        if !dir.exists() {
            return Ok(rel);
        }
        candidate_name = format!("{}-{}", base_name, counter);
        counter += 1;
    }
}

pub fn is_document_dir(path: &Path) -> bool {
    manifest_path(path).exists()
}

pub fn is_note_in_document(notes_root: &Path, note_id: &str) -> bool {
    if let Some((document_path, file)) = file_from_note_id(note_id) {
        if validate_page_file(&file).is_err() {
            return false;
        }
        if let Ok(dir) = document_dir(notes_root, &document_path) {
            return is_document_dir(&dir);
        }
    }
    false
}

pub fn desired_page_id_for_save(
    notes_root: &Path,
    existing_id: &str,
    title: &str,
) -> Result<Option<String>, String> {
    let Some((document_path, file)) = file_from_note_id(existing_id) else {
        return Ok(None);
    };
    let dir = document_dir(notes_root, &document_path)?;
    if !is_document_dir(&dir) {
        return Ok(None);
    }
    let prefix = file
        .split_once('-')
        .map(|(prefix, _)| prefix)
        .filter(|prefix| prefix.len() == 3 && prefix.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or("001");
    let desired_file = format!("{}-{}.md", prefix, crate::sanitize_filename(title));
    Ok(Some(page_id(&document_path, &desired_file)))
}

pub fn sync_page_after_save(
    notes_root: &Path,
    old_id: Option<&str>,
    new_id: &str,
    title: &str,
) -> Result<(), String> {
    let Some((document_path, new_file)) = file_from_note_id(new_id) else {
        return Ok(());
    };
    let dir = document_dir(notes_root, &document_path)?;
    if !is_document_dir(&dir) {
        return Ok(());
    }
    let mut manifest = read_manifest(&dir)?;
    let old_file = old_id.and_then(|id| file_from_note_id(id).map(|(_, f)| f));
    let mut found = false;
    for page in &mut manifest.pages {
        if page.file == new_file || old_file.as_deref() == Some(page.file.as_str()) {
            page.file = new_file.clone();
            page.title = title.to_string();
            found = true;
            break;
        }
    }
    if !found {
        manifest.pages.push(DocumentPageManifest {
            file: new_file,
            title: title.to_string(),
        });
    }
    manifest.updated_at = now_string();
    write_manifest(&dir, &manifest)
}

pub fn create_document(
    notes_root: &Path,
    parent_path: Option<String>,
    title: String,
) -> Result<DocumentDetail, String> {
    let clean_title = if title.trim().is_empty() {
        "Untitled Document".to_string()
    } else {
        title.trim().to_string()
    };
    let document_path = ensure_unique_folder(notes_root, parent_path.as_deref(), &clean_title)?;
    let dir = document_dir(notes_root, &document_path)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let first_file = page_file_name(1, "Page 1");
    fs::write(dir.join(&first_file), "").map_err(|e| e.to_string())?;

    let now = now_string();
    let manifest = DocumentManifest {
        document_type: DOCUMENT_TYPE.to_string(),
        version: 1,
        title: clean_title,
        pages: vec![DocumentPageManifest {
            file: first_file,
            title: "Page 1".to_string(),
        }],
        created_at: now.clone(),
        updated_at: now,
    };
    write_manifest(&dir, &manifest)?;
    read_document(notes_root, &document_path, 800)
}

pub fn rename_document(
    notes_root: &Path,
    document_path: &str,
    new_name: String,
    word_limit: usize,
) -> Result<DocumentDetail, String> {
    validate_document_path(document_path)?;
    if new_name.trim().is_empty() {
        return Err("Document name cannot be empty".to_string());
    }
    let clean_name = crate::sanitize_filename(new_name.trim());

    let old_path = normalize_rel_path(document_path);
    let old_dir = document_dir(notes_root, &old_path)?;
    if !old_dir.is_dir() || !is_document_dir(&old_dir) {
        return Err("Path is not a Markch Document".to_string());
    }

    let mut manifest = read_manifest(&old_dir)?;
    let parent_path = old_path.rsplit_once('/').map(|(parent, _)| parent);
    let new_path = if let Some(parent) = parent_path {
        format!("{}/{}", parent, clean_name)
    } else {
        clean_name.clone()
    };
    let new_dir = document_dir(notes_root, &new_path)?;
    if new_dir.exists() {
        return Err("A folder with that name already exists".to_string());
    }

    fs::rename(&old_dir, &new_dir).map_err(|e| e.to_string())?;
    manifest.title = clean_name;
    manifest.updated_at = now_string();
    write_manifest(&new_dir, &manifest)?;
    read_document(notes_root, &new_path, word_limit)
}

pub fn delete_document(notes_root: &Path, document_path: &str) -> Result<(), String> {
    validate_document_path(document_path)?;
    let dir = document_dir(notes_root, document_path)?;
    if !dir.is_dir() || !is_document_dir(&dir) {
        return Err("Path is not a Markch Document".to_string());
    }
    fs::remove_dir_all(&dir).map_err(|e| e.to_string())
}

pub fn list_documents(
    notes_root: &Path,
    ignored_dirs: &[String],
) -> Result<Vec<DocumentMetadata>, String> {
    let mut documents = Vec::new();
    for entry in walkdir::WalkDir::new(notes_root)
        .max_depth(10)
        .into_iter()
        .filter_entry(|entry| crate::is_visible_notes_entry(entry, ignored_dirs))
        .flatten()
    {
        if !entry.file_type().is_dir() || !is_document_dir(entry.path()) {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(notes_root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let Ok(manifest) = read_manifest(entry.path()) else {
            continue;
        };
        documents.push(DocumentMetadata {
            path: rel,
            title: manifest.title,
            page_count: manifest.pages.len(),
            modified: modified_seconds(&manifest_path(entry.path())),
        });
    }
    documents.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(documents)
}

pub fn read_document(
    notes_root: &Path,
    document_path: &str,
    word_limit: usize,
) -> Result<DocumentDetail, String> {
    let dir = document_dir(notes_root, document_path)?;
    let manifest = read_manifest(&dir)?;
    let mut pages = Vec::new();
    for (idx, page) in manifest.pages.iter().enumerate() {
        validate_page_file(&page.file)?;
        let file_path = dir.join(&page.file);
        let content = fs::read_to_string(&file_path).unwrap_or_default();
        let word_count = count_words(&content);
        pages.push(DocumentPage {
            id: page_id(document_path, &page.file),
            file: page.file.clone(),
            title: page.title.clone(),
            modified: modified_seconds(&file_path),
            index: idx + 1,
            word_count,
            overflow: word_count > word_limit,
        });
    }
    Ok(DocumentDetail {
        path: normalize_rel_path(document_path),
        title: manifest.title,
        pages,
    })
}

pub fn read_document_markdown(notes_root: &Path, document_path: &str) -> Result<String, String> {
    let dir = document_dir(notes_root, document_path)?;
    if !is_document_dir(&dir) {
        return Err("Document not found".to_string());
    }
    let manifest = read_manifest(&dir)?;
    let mut markdown = String::new();
    for page in manifest.pages {
        validate_page_file(&page.file)?;
        let file_path = dir.join(page.file);
        let content = fs::read_to_string(file_path).map_err(|e| e.to_string())?;
        markdown.push_str(&content);
    }
    Ok(markdown)
}

pub fn read_document_edit_markdown(
    notes_root: &Path,
    document_path: &str,
) -> Result<String, String> {
    read_document_markdown_with_boundaries(notes_root, document_path)
}

fn read_document_markdown_for_normalize(
    notes_root: &Path,
    document_path: &str,
) -> Result<String, String> {
    read_document_markdown_with_boundaries(notes_root, document_path)
}

fn read_document_markdown_with_boundaries(
    notes_root: &Path,
    document_path: &str,
) -> Result<String, String> {
    let dir = document_dir(notes_root, document_path)?;
    if !is_document_dir(&dir) {
        return Err("Document not found".to_string());
    }
    let manifest = read_manifest(&dir)?;
    let mut pages = Vec::new();
    for page in manifest.pages {
        validate_page_file(&page.file)?;
        let file_path = dir.join(page.file);
        pages.push(fs::read_to_string(file_path).map_err(|e| e.to_string())?);
    }
    Ok(join_markdown_pages_for_normalize(&pages))
}

fn join_markdown_pages_for_normalize(pages: &[String]) -> String {
    pages
        .iter()
        .map(|page| page.trim())
        .filter(|page| !page.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn save_document_markdown(
    notes_root: &Path,
    document_path: &str,
    markdown: &str,
    word_limit: usize,
) -> Result<DocumentDetail, String> {
    write_paginated_document(notes_root, document_path, markdown, word_limit)
        .map(|result| result.document)
}

pub fn read_document_for_note(
    notes_root: &Path,
    note_id: &str,
    word_limit: usize,
) -> Result<Option<DocumentDetail>, String> {
    let Some((document_path, file)) = file_from_note_id(note_id) else {
        return Ok(None);
    };
    let dir = document_dir(notes_root, &document_path)?;
    if !is_document_dir(&dir) {
        return Ok(None);
    }
    validate_page_file(&file)?;
    read_document(notes_root, &document_path, word_limit).map(Some)
}

pub fn create_document_page(
    notes_root: &Path,
    document_path: &str,
    title: Option<String>,
) -> Result<DocumentDetail, String> {
    let dir = document_dir(notes_root, document_path)?;
    let mut manifest = read_manifest(&dir)?;
    let index = manifest.pages.len() + 1;
    let explicit_title = title
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let title = explicit_title
        .clone()
        .unwrap_or_else(|| format!("Page {}", index));
    let file = page_file_name(index, &title);
    let content = explicit_title
        .map(|value| format!("# {}\n\n", value))
        .unwrap_or_default();
    fs::write(dir.join(&file), content).map_err(|e| e.to_string())?;
    manifest.pages.push(DocumentPageManifest { file, title });
    manifest.updated_at = now_string();
    write_manifest(&dir, &manifest)?;
    read_document(notes_root, document_path, 800)
}

pub fn rename_document_page(
    notes_root: &Path,
    document_path: &str,
    page_file: &str,
    title: String,
) -> Result<DocumentDetail, String> {
    validate_page_file(page_file)?;
    let dir = document_dir(notes_root, document_path)?;
    let mut manifest = read_manifest(&dir)?;
    let Some(index) = manifest
        .pages
        .iter()
        .position(|page| page.file == page_file)
    else {
        return Err("Page not found".to_string());
    };
    let clean_title = if title.trim().is_empty() {
        format!("Page {}", index + 1)
    } else {
        title.trim().to_string()
    };
    let new_file = page_file_name(index + 1, &clean_title);
    if new_file != page_file {
        fs::rename(dir.join(page_file), dir.join(&new_file)).map_err(|e| e.to_string())?;
    }
    manifest.pages[index].file = new_file;
    manifest.pages[index].title = clean_title;
    manifest.updated_at = now_string();
    write_manifest(&dir, &manifest)?;
    read_document(notes_root, document_path, 800)
}

pub fn delete_document_page(
    notes_root: &Path,
    document_path: &str,
    page_file: &str,
) -> Result<DocumentDetail, String> {
    validate_page_file(page_file)?;
    let dir = document_dir(notes_root, document_path)?;
    let mut manifest = read_manifest(&dir)?;
    if manifest.pages.len() <= 1 {
        return Err("A Document must keep at least one page".to_string());
    }
    let Some(index) = manifest
        .pages
        .iter()
        .position(|page| page.file == page_file)
    else {
        return Err("Page not found".to_string());
    };
    let removed = manifest.pages.remove(index);
    let _ = fs::remove_file(dir.join(removed.file));
    renumber_pages(&dir, &mut manifest)?;
    manifest.updated_at = now_string();
    write_manifest(&dir, &manifest)?;
    read_document(notes_root, document_path, 800)
}

pub fn move_document_page(
    notes_root: &Path,
    document_path: &str,
    page_file: &str,
    direction: MoveDirection,
) -> Result<DocumentDetail, String> {
    validate_page_file(page_file)?;
    let dir = document_dir(notes_root, document_path)?;
    let mut manifest = read_manifest(&dir)?;
    let Some(index) = manifest
        .pages
        .iter()
        .position(|page| page.file == page_file)
    else {
        return Err("Page not found".to_string());
    };
    let target_index = match direction {
        MoveDirection::Up if index > 0 => index - 1,
        MoveDirection::Down if index + 1 < manifest.pages.len() => index + 1,
        _ => return read_document(notes_root, document_path, 800),
    };
    manifest.pages.swap(index, target_index);
    renumber_pages(&dir, &mut manifest)?;
    manifest.updated_at = now_string();
    write_manifest(&dir, &manifest)?;
    read_document(notes_root, document_path, 800)
}

fn renumber_pages(dir: &Path, manifest: &mut DocumentManifest) -> Result<(), String> {
    let mut temp_files = Vec::new();
    for page in &mut manifest.pages {
        validate_page_file(&page.file)?;
        let temp = format!("{}.markch-tmp", page.file);
        fs::rename(dir.join(&page.file), dir.join(&temp)).map_err(|e| e.to_string())?;
        temp_files.push(temp);
    }
    for (idx, page) in manifest.pages.iter_mut().enumerate() {
        let new_file = page_file_name(idx + 1, &page.title);
        let temp = &temp_files[idx];
        fs::rename(dir.join(temp), dir.join(&new_file)).map_err(|e| e.to_string())?;
        page.file = new_file;
    }
    Ok(())
}

pub fn normalize_document_for_note_id(
    notes_root: &Path,
    note_id: &str,
    word_limit: usize,
) -> Result<Option<DocumentNormalizeResult>, String> {
    let Some((document_path, file)) = file_from_note_id(note_id) else {
        return Ok(None);
    };
    let dir = document_dir(notes_root, &document_path)?;
    if !is_document_dir(&dir) {
        return Ok(None);
    }
    let manifest = read_manifest(&dir)?;
    let page_index = page_index_for_file(&manifest, &file);
    let mut result = normalize_document(notes_root, &document_path, word_limit)?;
    result.target_note_id = target_note_id_for_index(&result.document, page_index);
    Ok(Some(result))
}

pub fn normalize_document(
    notes_root: &Path,
    document_path: &str,
    word_limit: usize,
) -> Result<DocumentNormalizeResult, String> {
    let markdown = read_document_markdown_for_normalize(notes_root, document_path)?;
    let result = write_paginated_document(notes_root, document_path, &markdown, word_limit)?;
    Ok(DocumentNormalizeResult {
        document: result.document,
        changed: result.changed,
        target_note_id: None,
    })
}

fn write_paginated_document(
    notes_root: &Path,
    document_path: &str,
    markdown: &str,
    word_limit: usize,
) -> Result<PaginatedWriteResult, String> {
    let word_limit = word_limit.clamp(250, 2000);
    let dir = document_dir(notes_root, document_path)?;
    let manifest = read_manifest(&dir)?;
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let old_files = manifest
        .pages
        .iter()
        .map(|page| page.file.clone())
        .collect::<Vec<_>>();
    for file in &old_files {
        validate_page_file(file)?;
    }

    let drafts = paginate_markdown(markdown, word_limit);
    let next_pages = drafts
        .iter()
        .enumerate()
        .map(|(idx, draft)| {
            let file = page_file_name(idx + 1, &draft.title);
            Ok(DocumentPageManifest {
                file,
                title: draft.title.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let old_file_set = old_files
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    for page in &next_pages {
        let final_path = dir.join(&page.file);
        if final_path.exists() && !old_file_set.contains(&page.file) {
            return Err(format!("Document page file already exists: {}", page.file));
        }
    }

    if manifest.pages == next_pages && page_contents_match_drafts(&dir, &next_pages, &drafts)? {
        return Ok(PaginatedWriteResult {
            document: read_document(notes_root, document_path, word_limit)?,
            changed: false,
        });
    }

    let mut temp_pages = Vec::new();
    for (idx, draft) in drafts.iter().enumerate() {
        let temp_file = format!(".markch-write-{}-{:03}.md.tmp", nonce, idx + 1);
        if let Err(error) = fs::write(dir.join(&temp_file), &draft.content) {
            cleanup_temp_files(&dir, &temp_pages, None);
            return Err(error.to_string());
        }
        temp_pages.push(temp_file);
    }

    let temp_manifest_file = format!("{}.write-{}.tmp", MANIFEST_FILE, nonce);
    let mut next_manifest = manifest.clone();
    next_manifest.pages = next_pages;
    next_manifest.updated_at = now_string();
    let temp_manifest_path = dir.join(&temp_manifest_file);
    let temp_manifest = serde_json::to_string_pretty(&next_manifest).map_err(|e| e.to_string())?;
    fs::write(&temp_manifest_path, temp_manifest).map_err(|e| {
        cleanup_temp_files(&dir, &temp_pages, Some(&temp_manifest_file));
        e.to_string()
    })?;

    let result = publish_paginated_document(&dir, &manifest, &next_manifest, &temp_pages, &nonce);
    if result.is_err() {
        cleanup_temp_files(&dir, &temp_pages, Some(&temp_manifest_file));
    }
    result?;
    Ok(PaginatedWriteResult {
        document: read_document(notes_root, document_path, word_limit)?,
        changed: true,
    })
}

fn page_contents_match_drafts(
    dir: &Path,
    pages: &[DocumentPageManifest],
    drafts: &[PageDraft],
) -> Result<bool, String> {
    if pages.len() != drafts.len() {
        return Ok(false);
    }
    for (page, draft) in pages.iter().zip(drafts.iter()) {
        validate_page_file(&page.file)?;
        let content = fs::read_to_string(dir.join(&page.file)).map_err(|e| e.to_string())?;
        if content != draft.content {
            return Ok(false);
        }
    }
    Ok(true)
}

fn publish_paginated_document(
    dir: &Path,
    old_manifest: &DocumentManifest,
    next_manifest: &DocumentManifest,
    temp_pages: &[String],
    nonce: &str,
) -> Result<(), String> {
    let mut page_backups: Vec<(String, String)> = Vec::new();
    let manifest_backup = format!("{}.backup-{}.tmp", MANIFEST_FILE, nonce);

    for page in &old_manifest.pages {
        validate_page_file(&page.file)?;
        let path = dir.join(&page.file);
        if !path.exists() {
            continue;
        }
        let backup = format!(".markch-old-{}-{}", nonce, page.file);
        fs::rename(&path, dir.join(&backup)).map_err(|e| e.to_string())?;
        page_backups.push((page.file.clone(), backup));
    }

    let publish_result = (|| -> Result<(), String> {
        for (page, temp_file) in next_manifest.pages.iter().zip(temp_pages.iter()) {
            validate_page_file(&page.file)?;
            fs::rename(dir.join(temp_file), dir.join(&page.file)).map_err(|e| e.to_string())?;
        }
        fs::rename(manifest_path(dir), dir.join(&manifest_backup)).map_err(|e| e.to_string())?;
        fs::rename(
            dir.join(format!("{}.write-{}.tmp", MANIFEST_FILE, nonce)),
            manifest_path(dir),
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })();

    if let Err(error) = publish_result {
        rollback_paginated_document(dir, &page_backups, next_manifest, &manifest_backup);
        return Err(error);
    }

    for (_, backup) in page_backups {
        let _ = fs::remove_file(dir.join(backup));
    }
    let _ = fs::remove_file(dir.join(manifest_backup));
    Ok(())
}

fn rollback_paginated_document(
    dir: &Path,
    page_backups: &[(String, String)],
    next_manifest: &DocumentManifest,
    manifest_backup: &str,
) {
    for page in &next_manifest.pages {
        let _ = fs::remove_file(dir.join(&page.file));
    }
    if dir.join(manifest_backup).exists() {
        let _ = fs::remove_file(manifest_path(dir));
        let _ = fs::rename(dir.join(manifest_backup), manifest_path(dir));
    }
    for (original, backup) in page_backups.iter().rev() {
        if dir.join(backup).exists() {
            let _ = fs::rename(dir.join(backup), dir.join(original));
        }
    }
}

fn cleanup_temp_files(dir: &Path, temp_pages: &[String], temp_manifest: Option<&str>) {
    for temp in temp_pages {
        let _ = fs::remove_file(dir.join(temp));
    }
    if let Some(temp_manifest) = temp_manifest {
        let _ = fs::remove_file(dir.join(temp_manifest));
    }
}

fn count_words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .count()
}

fn classify_block(text: &str) -> BlockKind {
    let trimmed = text.trim_start();
    if heading_title(trimmed).is_some() {
        BlockKind::Heading
    } else if trimmed.starts_with("```")
        || trimmed.starts_with('|')
        || trimmed.starts_with("![")
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
        || trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
    {
        BlockKind::Atomic
    } else {
        BlockKind::Paragraph
    }
}

fn split_markdown_blocks(content: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    let mut in_code = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            current.push(line.to_string());
            in_code = !in_code;
            if !in_code {
                push_block(&mut blocks, &mut current);
            }
            continue;
        }

        if in_code {
            current.push(line.to_string());
            continue;
        }

        if heading_title(trimmed).is_some() {
            push_block(&mut blocks, &mut current);
            current.push(line.to_string());
            push_block(&mut blocks, &mut current);
        } else if trimmed.is_empty() {
            push_block(&mut blocks, &mut current);
        } else {
            current.push(line.to_string());
        }
    }
    push_block(&mut blocks, &mut current);
    blocks
}

fn paginate_markdown(markdown: &str, limit: usize) -> Vec<PageDraft> {
    let blocks = split_markdown_blocks(markdown);
    if blocks.is_empty() {
        return vec![PageDraft {
            title: "Page 1".to_string(),
            content: String::new(),
        }];
    }

    let mut pages = Vec::new();
    let mut current = Vec::new();
    let mut count = 0usize;
    let heading_break_threshold = (limit / 2).max(1);

    for block in blocks {
        let should_break_for_limit =
            !current.is_empty() && count + block.words > limit && !has_only_heading(&current);
        let should_break_for_heading = matches!(block.kind, BlockKind::Heading)
            && !current.is_empty()
            && count >= heading_break_threshold;

        if should_break_for_limit || should_break_for_heading {
            pages.push(compose_page_draft(pages.len() + 1, &current));
            current.clear();
            count = 0;
        }

        count += block.words;
        current.push(block);
    }

    if !current.is_empty() {
        pages.push(compose_page_draft(pages.len() + 1, &current));
    }

    if pages.is_empty() {
        vec![PageDraft {
            title: "Page 1".to_string(),
            content: String::new(),
        }]
    } else {
        pages
    }
}

fn has_only_heading(blocks: &[MarkdownBlock]) -> bool {
    blocks.len() == 1 && matches!(blocks[0].kind, BlockKind::Heading)
}

fn compose_page_draft(index: usize, blocks: &[MarkdownBlock]) -> PageDraft {
    let title = blocks
        .iter()
        .find_map(|block| heading_title(&block.text))
        .unwrap_or_else(|| format!("Page {}", index));
    let mut content = blocks
        .iter()
        .map(|block| block.text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }

    PageDraft { title, content }
}

fn heading_title(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = trimmed.get(hashes..)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let title = rest.trim().trim_end_matches('#').trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn push_block(blocks: &mut Vec<MarkdownBlock>, current: &mut Vec<String>) {
    let text = current.join("\n").trim().to_string();
    current.clear();
    if text.is_empty() {
        return;
    }
    let words = count_words(&text);
    let kind = classify_block(&text);
    blocks.push(MarkdownBlock { text, words, kind });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "markch-document-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn normalize_detail(root: &Path, document_path: &str, word_limit: usize) -> DocumentDetail {
        normalize_document(root, document_path, word_limit)
            .unwrap()
            .document
    }

    #[test]
    fn creates_and_reads_manifest() {
        let root = temp_root("manifest");
        let detail = create_document(&root, None, "Project Alpha".to_string()).unwrap();
        assert_eq!(detail.title, "Project Alpha");
        assert_eq!(detail.pages.len(), 1);
        assert!(root.join("Project Alpha").join(MANIFEST_FILE).exists());
        let reread = read_document(&root, "Project Alpha", 800).unwrap();
        assert_eq!(reread.pages[0].file, "001-Page 1.md");
    }

    #[test]
    fn automatic_page_titles_do_not_write_synthetic_headers() {
        let root = temp_root("automatic-page-title");
        create_document(&root, None, "Doc".to_string()).unwrap();
        create_document_page(&root, "Doc", None).unwrap();

        assert_eq!(
            fs::read_to_string(root.join("Doc").join("001-Page 1.md")).unwrap(),
            ""
        );
        assert_eq!(
            fs::read_to_string(root.join("Doc").join("002-Page 2.md")).unwrap(),
            ""
        );
    }

    #[test]
    fn rejects_escaping_document_paths() {
        let root = temp_root("escape");
        let result = read_document(&root, "../outside", 800);
        assert!(result.is_err());
    }

    #[test]
    fn read_document_markdown_rejects_escaping_paths() {
        let root = temp_root("markdown-escape");
        let result = read_document_markdown(&root, "../outside");
        assert!(result.is_err());
    }

    #[test]
    fn list_documents_skips_invalid_manifest_and_keeps_valid_documents() {
        let root = temp_root("list-documents-invalid-manifest");
        create_document(&root, None, "Valid".to_string()).unwrap();
        fs::create_dir_all(root.join("Broken")).unwrap();
        fs::write(root.join("Broken").join(MANIFEST_FILE), "{ invalid json").unwrap();

        let documents = list_documents(&root, &[]).unwrap();

        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].path, "Valid");
    }

    #[test]
    fn detects_document_by_manifest() {
        let root = temp_root("detect");
        create_document(&root, Some("Parent".to_string()), "Doc".to_string()).unwrap();
        assert!(is_note_in_document(&root, "Parent/Doc/001-Page 1"));
        assert!(!is_note_in_document(&root, "Parent/Other/001-Page 1"));
    }

    #[test]
    fn renames_document_folder_and_manifest_title() {
        let root = temp_root("rename-document");
        create_document(&root, Some("Parent".to_string()), "Draft".to_string()).unwrap();
        fs::write(
            root.join("Parent").join("Draft").join("001-Page 1.md"),
            "# First\n\nBody\n",
        )
        .unwrap();

        let detail =
            rename_document(&root, "Parent/Draft", "Final Draft".to_string(), 800).unwrap();

        assert_eq!(detail.path, "Parent/Final Draft");
        assert_eq!(detail.title, "Final Draft");
        assert!(root
            .join("Parent")
            .join("Final Draft")
            .join(MANIFEST_FILE)
            .exists());
        assert!(!root.join("Parent").join("Draft").exists());
        assert_eq!(detail.pages[0].id, "Parent/Final Draft/001-Page 1");
        let manifest =
            read_manifest(&root.join("Parent").join("Final Draft")).expect("manifest after rename");
        assert_eq!(manifest.title, "Final Draft");
        assert_eq!(
            fs::read_to_string(
                root.join("Parent")
                    .join("Final Draft")
                    .join("001-Page 1.md")
            )
            .unwrap(),
            "# First\n\nBody\n"
        );
    }

    #[test]
    fn rename_document_rejects_escaping_paths() {
        let root = temp_root("rename-document-escape");

        let result = rename_document(&root, "../Doc", "Other".to_string(), 800);

        assert!(result.is_err());
    }

    #[test]
    fn rename_document_rejects_collisions() {
        let root = temp_root("rename-document-collision");
        create_document(&root, None, "Draft".to_string()).unwrap();
        fs::create_dir_all(root.join("Existing")).unwrap();

        let result = rename_document(&root, "Draft", "Existing".to_string(), 800);

        assert!(result.is_err());
        assert!(root.join("Draft").exists());
        assert!(root.join("Existing").exists());
    }

    #[test]
    fn delete_document_removes_valid_document_folder() {
        let root = temp_root("delete-document");
        create_document(&root, None, "Doc".to_string()).unwrap();
        create_document_page(&root, "Doc", Some("Second".to_string())).unwrap();

        delete_document(&root, "Doc").unwrap();

        assert!(!root.join("Doc").exists());
    }

    #[test]
    fn delete_document_rejects_escaping_paths() {
        let root = temp_root("delete-document-escape");

        let result = delete_document(&root, "../outside");

        assert!(result.is_err());
    }

    #[test]
    fn delete_document_rejects_regular_folders() {
        let root = temp_root("delete-document-regular-folder");
        fs::create_dir_all(root.join("Regular")).unwrap();
        fs::write(root.join("Regular").join("note.md"), "body").unwrap();

        let result = delete_document(&root, "Regular");

        assert!(result.is_err());
        assert!(root.join("Regular").exists());
        assert!(root.join("Regular").join("note.md").exists());
    }

    #[test]
    fn delete_document_does_not_affect_other_documents_or_notes() {
        let root = temp_root("delete-document-isolated");
        create_document(&root, None, "Doc A".to_string()).unwrap();
        create_document(&root, None, "Doc B".to_string()).unwrap();
        fs::write(root.join("normal.md"), "normal").unwrap();

        delete_document(&root, "Doc A").unwrap();

        assert!(!root.join("Doc A").exists());
        assert!(root.join("Doc B").join(MANIFEST_FILE).exists());
        assert!(root.join("normal.md").exists());
    }

    #[test]
    fn preserves_manifest_order() {
        let root = temp_root("order");
        create_document(&root, None, "Doc".to_string()).unwrap();
        create_document_page(&root, "Doc", Some("Second".to_string())).unwrap();
        let detail = read_document(&root, "Doc", 800).unwrap();
        assert_eq!(detail.pages[0].index, 1);
        assert_eq!(detail.pages[1].index, 2);
        assert_eq!(detail.pages[1].title, "Second");
    }

    #[test]
    fn read_document_markdown_concatenates_pages_in_manifest_order() {
        let root = temp_root("markdown-order");
        create_document(&root, None, "Doc".to_string()).unwrap();
        create_document_page(&root, "Doc", Some("Second".to_string())).unwrap();
        fs::write(root.join("Doc").join("001-Page 1.md"), "# First\n\nA\n").unwrap();
        fs::write(root.join("Doc").join("002-Second.md"), "# Second\n\nB\n").unwrap();

        let markdown = read_document_markdown(&root, "Doc").unwrap();

        assert_eq!(markdown, "# First\n\nA\n# Second\n\nB\n");
    }

    #[test]
    fn read_document_markdown_does_not_insert_extra_separators() {
        let root = temp_root("markdown-no-separators");
        create_document(&root, None, "Doc".to_string()).unwrap();
        create_document_page(&root, "Doc", Some("Second".to_string())).unwrap();
        fs::write(root.join("Doc").join("001-Page 1.md"), "one").unwrap();
        fs::write(root.join("Doc").join("002-Second.md"), "two").unwrap();

        let markdown = read_document_markdown(&root, "Doc").unwrap();

        assert_eq!(markdown, "onetwo");
    }

    #[test]
    fn read_document_edit_markdown_inserts_stable_page_boundaries() {
        let root = temp_root("edit-markdown-boundaries");
        create_document(&root, None, "Doc".to_string()).unwrap();
        create_document_page(&root, "Doc", Some("Second".to_string())).unwrap();
        fs::write(root.join("Doc").join("001-Page 1.md"), "one").unwrap();
        fs::write(root.join("Doc").join("002-Second.md"), "two").unwrap();

        let markdown = read_document_edit_markdown(&root, "Doc").unwrap();

        assert_eq!(markdown, "one\n\ntwo");
    }

    #[test]
    fn normalize_reads_pages_with_stable_markdown_boundaries() {
        let root = temp_root("normalize-boundaries");
        create_document(&root, None, "Doc".to_string()).unwrap();
        create_document_page(&root, "Doc", Some("Second".to_string())).unwrap();
        fs::write(root.join("Doc").join("001-Page 1.md"), "one").unwrap();
        fs::write(root.join("Doc").join("002-Second.md"), "two").unwrap();

        let detail = normalize_detail(&root, "Doc", 250);
        let content = fs::read_to_string(root.join("Doc").join(&detail.pages[0].file)).unwrap();

        assert_eq!(detail.pages.len(), 1);
        assert_eq!(content, "one\n\ntwo\n");
    }

    #[test]
    fn paginates_markdown_into_multiple_pages() {
        let root = temp_root("paginate");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let page = root.join("Doc").join("001-Page 1.md");
        let first_words = (0..240)
            .map(|i| format!("alpha{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        let second_words = (0..80)
            .map(|i| format!("zeta{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        fs::write(
            &page,
            format!("# Page 1\n\n{}\n\n{}\n", first_words, second_words),
        )
        .unwrap();
        let detail = normalize_detail(&root, "Doc", 250);
        assert_eq!(detail.pages.len(), 2);
        let first = fs::read_to_string(root.join("Doc").join(&detail.pages[0].file)).unwrap();
        let second = fs::read_to_string(root.join("Doc").join(&detail.pages[1].file)).unwrap();
        assert!(first.contains("alpha0"));
        assert!(second.contains("zeta0"));
    }

    #[test]
    fn header_can_start_a_new_page_near_limit() {
        let root = temp_root("header-break");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let page = root.join("Doc").join("001-Page 1.md");
        let intro = (0..140)
            .map(|i| format!("intro{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        fs::write(
            &page,
            format!("# Intro\n\n{}\n\n## Next Section\n\nshort body\n", intro),
        )
        .unwrap();

        let detail = normalize_detail(&root, "Doc", 250);

        assert_eq!(detail.pages.len(), 2);
        assert_eq!(detail.pages[1].title, "Next Section");
        let second = fs::read_to_string(root.join("Doc").join(&detail.pages[1].file)).unwrap();
        assert!(second.starts_with("## Next Section"));
    }

    #[test]
    fn page_title_comes_from_first_visible_header() {
        let root = temp_root("header-title");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let page = root.join("Doc").join("001-Page 1.md");
        fs::write(&page, "## Section Title\n\nbody\n").unwrap();

        let detail = normalize_detail(&root, "Doc", 250);

        assert_eq!(detail.pages[0].title, "Section Title");
        assert_eq!(detail.pages[0].file, "001-Section Title.md");
    }

    #[test]
    fn page_without_heading_uses_metadata_title_without_inserting_header() {
        let root = temp_root("metadata-title");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let page = root.join("Doc").join("001-Page 1.md");
        fs::write(&page, "plain body\n").unwrap();

        let detail = normalize_detail(&root, "Doc", 250);
        let content = fs::read_to_string(root.join("Doc").join(&detail.pages[0].file)).unwrap();

        assert_eq!(detail.pages[0].title, "Page 1");
        assert_eq!(content, "plain body\n");
        assert!(!content.starts_with("# Page 1"));
    }

    #[test]
    fn normalize_is_idempotent_after_first_write() {
        let root = temp_root("normalize-idempotent");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let body = (0..150)
            .map(|i| format!("word{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        fs::write(
            root.join("Doc").join("001-Page 1.md"),
            format!("# First\n\n{}\n\n## Second\n\nbody\n", body),
        )
        .unwrap();

        let first = normalize_document(&root, "Doc", 250).unwrap();
        let first_files = first
            .document
            .pages
            .iter()
            .map(|page| {
                (
                    page.file.clone(),
                    fs::read_to_string(root.join("Doc").join(&page.file)).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let second = normalize_document(&root, "Doc", 250).unwrap();
        let second_files = second
            .document
            .pages
            .iter()
            .map(|page| {
                (
                    page.file.clone(),
                    fs::read_to_string(root.join("Doc").join(&page.file)).unwrap(),
                )
            })
            .collect::<Vec<_>>();

        assert!(first.changed);
        assert!(!second.changed);
        assert_eq!(first.document.pages.len(), second.document.pages.len());
        assert_eq!(first_files, second_files);
    }

    #[test]
    fn normalize_for_note_returns_equivalent_target_note_id() {
        let root = temp_root("normalize-note-target");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let body = (0..150)
            .map(|i| format!("word{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        fs::write(
            root.join("Doc").join("001-Page 1.md"),
            format!("# First\n\n{}\n\n## Second\n\nbody\n", body),
        )
        .unwrap();

        let result = normalize_document_for_note_id(&root, "Doc/001-Page 1", 250)
            .unwrap()
            .unwrap();

        assert_eq!(result.target_note_id.as_deref(), Some("Doc/001-First"));
        assert!(root.join("Doc").join("001-First.md").exists());
    }

    #[test]
    fn read_document_for_note_recovers_from_obsolete_page_id() {
        let root = temp_root("obsolete-page-id");
        create_document(&root, None, "Doc".to_string()).unwrap();
        fs::write(root.join("Doc").join("001-Page 1.md"), "# First\n\nbody\n").unwrap();
        normalize_document(&root, "Doc", 250).unwrap();

        let detail = read_document_for_note(&root, "Doc/001-Page 1", 250)
            .unwrap()
            .unwrap();

        assert_eq!(detail.path, "Doc");
        assert_eq!(detail.pages[0].id, "Doc/001-First");
    }

    #[test]
    fn does_not_split_long_paragraph_mid_text() {
        let root = temp_root("long-paragraph");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let page = root.join("Doc").join("001-Page 1.md");
        let paragraph = (0..300)
            .map(|i| format!("word{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        fs::write(&page, format!("# Long\n\n{}\n", paragraph)).unwrap();

        let detail = normalize_detail(&root, "Doc", 250);
        let first = fs::read_to_string(root.join("Doc").join(&detail.pages[0].file)).unwrap();

        assert_eq!(detail.pages.len(), 1);
        assert!(detail.pages[0].overflow);
        assert!(first.contains("word0"));
        assert!(first.contains("word299"));
    }

    #[test]
    fn long_section_splits_by_blocks_without_losing_header() {
        let root = temp_root("long-section");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let page = root.join("Doc").join("001-Page 1.md");
        let first = (0..180)
            .map(|i| format!("first{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        let second = (0..120)
            .map(|i| format!("second{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        fs::write(&page, format!("# Alpha\n\n{}\n\n{}\n", first, second)).unwrap();

        let detail = normalize_detail(&root, "Doc", 250);
        let first_page = fs::read_to_string(root.join("Doc").join(&detail.pages[0].file)).unwrap();
        let second_page = fs::read_to_string(root.join("Doc").join(&detail.pages[1].file)).unwrap();

        assert!(first_page.starts_with("# Alpha"));
        assert!(first_page.contains("first0"));
        assert!(second_page.contains("second0"));
    }

    #[test]
    fn save_document_markdown_creates_renumbered_pages() {
        let root = temp_root("save-markdown");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let body = (0..150)
            .map(|i| format!("word{}", i))
            .collect::<Vec<_>>()
            .join(" ");

        let detail = save_document_markdown(
            &root,
            "Doc",
            &format!("# First\n\n{}\n\n## Second\n\nbody\n", body),
            250,
        )
        .unwrap();

        assert_eq!(detail.pages.len(), 2);
        assert_eq!(detail.pages[0].file, "001-First.md");
        assert_eq!(detail.pages[1].file, "002-Second.md");
    }

    #[test]
    fn save_document_markdown_leaves_no_temp_files_after_success() {
        let root = temp_root("save-clean-temp");
        create_document(&root, None, "Doc".to_string()).unwrap();

        save_document_markdown(&root, "Doc", "# First\n\nbody\n", 250).unwrap();

        let files = fs::read_dir(root.join("Doc"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(!files.iter().any(|file| file.contains(".markch-write-")));
        assert!(!files.iter().any(|file| file.contains(".markch-old-")));
        assert!(!files.iter().any(|file| file.contains(".backup-")));
    }

    #[test]
    fn save_document_markdown_preserves_old_pages_on_conflicting_output() {
        let root = temp_root("save-conflict");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let original_path = root.join("Doc").join("001-Page 1.md");
        fs::write(&original_path, "# Page 1\n\noriginal\n").unwrap();
        fs::write(root.join("Doc").join("002-Second.md"), "# Existing\n").unwrap();
        let body = (0..150)
            .map(|i| format!("word{}", i))
            .collect::<Vec<_>>()
            .join(" ");

        let result = save_document_markdown(
            &root,
            "Doc",
            &format!("# First\n\n{}\n\n## Second\n\nbody\n", body),
            250,
        );

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(original_path).unwrap(),
            "# Page 1\n\noriginal\n"
        );
    }

    #[test]
    fn save_document_markdown_rejects_escaping_paths() {
        let root = temp_root("save-escape");
        let result = save_document_markdown(&root, "../outside", "# Nope\n", 250);
        assert!(result.is_err());
    }

    #[test]
    fn keeps_atomic_block_when_it_exceeds_limit() {
        let root = temp_root("atomic");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let page = root.join("Doc").join("001-Page 1.md");
        let code_words = (0..260)
            .map(|i| format!("word{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        fs::write(
            &page,
            format!("# Page 1\n\n```txt\n{}\n```\n\nnext block\n", code_words),
        )
        .unwrap();
        let detail = normalize_detail(&root, "Doc", 250);
        let first = fs::read_to_string(root.join("Doc").join(&detail.pages[0].file)).unwrap();
        assert!(first.contains("```txt"));
        assert!(detail.pages[0].overflow);
    }
}
