use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST_FILE: &str = ".markch-document.json";
const DOCUMENT_TYPE: &str = "markch-document";

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Up,
    Down,
}

#[derive(Debug, Clone)]
enum BlockKind {
    Paragraph,
    Atomic,
}

#[derive(Debug, Clone)]
struct MarkdownBlock {
    text: String,
    words: usize,
    kind: BlockKind,
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

fn ensure_unique_folder(notes_root: &Path, parent: Option<&str>, title: &str) -> Result<String, String> {
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
    let first_content = "# Page 1\n\n";
    fs::write(dir.join(&first_file), first_content).map_err(|e| e.to_string())?;

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

pub fn list_documents(notes_root: &Path, ignored_dirs: &[String]) -> Result<Vec<DocumentMetadata>, String> {
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
        let manifest = read_manifest(entry.path())?;
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

pub fn read_document(notes_root: &Path, document_path: &str, word_limit: usize) -> Result<DocumentDetail, String> {
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
    let manifest = read_manifest(&dir)?;
    if !manifest.pages.iter().any(|page| page.file == file) {
        return Ok(None);
    }
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
    let title = title
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("Page {}", index));
    let file = page_file_name(index, &title);
    fs::write(dir.join(&file), format!("# {}\n\n", title)).map_err(|e| e.to_string())?;
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
    let Some(index) = manifest.pages.iter().position(|page| page.file == page_file) else {
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
    let Some(index) = manifest.pages.iter().position(|page| page.file == page_file) else {
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
    let Some(index) = manifest.pages.iter().position(|page| page.file == page_file) else {
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
) -> Result<Option<DocumentDetail>, String> {
    let Some((document_path, _)) = file_from_note_id(note_id) else {
        return Ok(None);
    };
    let dir = document_dir(notes_root, &document_path)?;
    if !is_document_dir(&dir) {
        return Ok(None);
    }
    normalize_document(notes_root, &document_path, word_limit).map(Some)
}

pub fn normalize_document(
    notes_root: &Path,
    document_path: &str,
    word_limit: usize,
) -> Result<DocumentDetail, String> {
    let word_limit = word_limit.clamp(250, 2000);
    let dir = document_dir(notes_root, document_path)?;
    let mut manifest = read_manifest(&dir)?;
    let mut carry: Vec<MarkdownBlock> = Vec::new();
    let mut index = 0;

    while index < manifest.pages.len() || !carry.is_empty() {
        if index >= manifest.pages.len() {
            let title = format!("Page {}", index + 1);
            let file = page_file_name(index + 1, &title);
            fs::write(dir.join(&file), format!("# {}\n\n", title)).map_err(|e| e.to_string())?;
            manifest.pages.push(DocumentPageManifest { file, title });
        }

        let page = manifest.pages[index].clone();
        let file_path = dir.join(&page.file);
        let original = fs::read_to_string(&file_path).unwrap_or_else(|_| format!("# {}\n\n", page.title));
        let (heading, body) = split_heading(&original, &page.title);
        let mut blocks = Vec::new();
        blocks.append(&mut carry);
        blocks.extend(split_markdown_blocks(&body));
        let (kept, overflow) = split_blocks_for_limit(blocks, word_limit);
        let next_content = compose_page(&heading, &kept);
        fs::write(&file_path, next_content).map_err(|e| e.to_string())?;
        carry = overflow;
        index += 1;
    }

    manifest.updated_at = now_string();
    write_manifest(&dir, &manifest)?;
    read_document(notes_root, document_path, word_limit)
}

fn split_heading(content: &str, fallback_title: &str) -> (String, String) {
    let mut lines = content.lines();
    if let Some(first) = lines.next() {
        if first.trim_start().starts_with("# ") {
            return (first.trim().to_string(), lines.collect::<Vec<_>>().join("\n"));
        }
    }
    (format!("# {}", fallback_title), content.to_string())
}

fn compose_page(heading: &str, blocks: &[MarkdownBlock]) -> String {
    let body = blocks
        .iter()
        .map(|block| block.text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if body.is_empty() {
        format!("{}\n\n", heading)
    } else {
        format!("{}\n\n{}\n", heading, body)
    }
}

fn count_words(text: &str) -> usize {
    text.split_whitespace().filter(|word| word.chars().any(char::is_alphanumeric)).count()
}

fn classify_block(text: &str) -> BlockKind {
    let trimmed = text.trim_start();
    if trimmed.starts_with("```")
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

        if trimmed.is_empty() {
            push_block(&mut blocks, &mut current);
        } else {
            current.push(line.to_string());
        }
    }
    push_block(&mut blocks, &mut current);
    blocks
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

fn split_blocks_for_limit(
    blocks: Vec<MarkdownBlock>,
    limit: usize,
) -> (Vec<MarkdownBlock>, Vec<MarkdownBlock>) {
    let mut kept = Vec::new();
    let mut overflow = Vec::new();
    let mut count = 0usize;
    let mut overflowing = false;

    for block in blocks {
        if overflowing {
            overflow.push(block);
            continue;
        }

        if count + block.words <= limit || kept.is_empty() {
            if kept.is_empty() && block.words > limit && matches!(block.kind, BlockKind::Paragraph) {
                let (head, tail) = split_paragraph_block(block, limit);
                count += head.words;
                kept.push(head);
                overflow.extend(tail);
                overflowing = true;
            } else {
                count += block.words;
                kept.push(block);
            }
        } else {
            overflow.push(block);
            overflowing = true;
        }
    }

    (kept, overflow)
}

fn split_paragraph_block(block: MarkdownBlock, limit: usize) -> (MarkdownBlock, Vec<MarkdownBlock>) {
    let words = block.text.split_whitespace().collect::<Vec<_>>();
    let head_words = words.iter().take(limit).copied().collect::<Vec<_>>().join(" ");
    let tail_words = words.iter().skip(limit).copied().collect::<Vec<_>>().join(" ");
    let head = MarkdownBlock {
        words: count_words(&head_words),
        text: head_words,
        kind: BlockKind::Paragraph,
    };
    let tail = if tail_words.is_empty() {
        Vec::new()
    } else {
        vec![MarkdownBlock {
            words: count_words(&tail_words),
            text: tail_words,
            kind: BlockKind::Paragraph,
        }]
    };
    (head, tail)
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
    fn detects_document_by_manifest() {
        let root = temp_root("detect");
        create_document(&root, Some("Parent".to_string()), "Doc".to_string()).unwrap();
        assert!(is_note_in_document(&root, "Parent/Doc/001-Page 1"));
        assert!(!is_note_in_document(&root, "Parent/Other/001-Page 1"));
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
    fn paginates_markdown_into_multiple_pages() {
        let root = temp_root("paginate");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let page = root.join("Doc").join("001-Page 1.md");
        let first_words = (0..240).map(|i| format!("alpha{}", i)).collect::<Vec<_>>().join(" ");
        let second_words = (0..80).map(|i| format!("zeta{}", i)).collect::<Vec<_>>().join(" ");
        fs::write(
            &page,
            format!("# Page 1\n\n{}\n\n{}\n", first_words, second_words),
        )
        .unwrap();
        let detail = normalize_document(&root, "Doc", 250).unwrap();
        assert_eq!(detail.pages.len(), 2);
        let first = fs::read_to_string(root.join("Doc").join(&detail.pages[0].file)).unwrap();
        let second = fs::read_to_string(root.join("Doc").join(&detail.pages[1].file)).unwrap();
        assert!(first.contains("alpha0"));
        assert!(second.contains("zeta0"));
    }

    #[test]
    fn keeps_atomic_block_when_it_exceeds_limit() {
        let root = temp_root("atomic");
        create_document(&root, None, "Doc".to_string()).unwrap();
        let page = root.join("Doc").join("001-Page 1.md");
        let code_words = (0..260).map(|i| format!("word{}", i)).collect::<Vec<_>>().join(" ");
        fs::write(
            &page,
            format!("# Page 1\n\n```txt\n{}\n```\n\nnext block\n", code_words),
        )
        .unwrap();
        let detail = normalize_document(&root, "Doc", 250).unwrap();
        let first = fs::read_to_string(root.join("Doc").join(&detail.pages[0].file)).unwrap();
        assert!(first.contains("```txt"));
        assert!(detail.pages[0].overflow);
    }
}
