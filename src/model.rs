//! Pages and blocks, and how they turn into plain Markdown files and back.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_BLOCK: AtomicU64 = AtomicU64::new(1);

pub fn next_block_id() -> u64 {
    NEXT_BLOCK.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    H1,
    H2,
    H3,
    Bullet,
    Numbered,
    Todo,
    Toggle,
    Quote,
    Callout,
    Code,
    Divider,
    Table,
    PageLink,
    /// Only used in the "/" menu: creates a sub-database and a link to it.
    DbLink,
    /// A picture stored in the workspace's `_assets` folder.
    Image,
    /// A link shown as a preview card.
    Bookmark,
}

impl Kind {
    pub const MENU: [Kind; 15] = [
        Kind::Text,
        Kind::H1,
        Kind::H2,
        Kind::H3,
        Kind::Bullet,
        Kind::Numbered,
        Kind::Todo,
        Kind::Toggle,
        Kind::Quote,
        Kind::Callout,
        Kind::Code,
        Kind::Divider,
        Kind::Table,
        Kind::Image,
        Kind::Bookmark,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Text => "Text",
            Kind::H1 => "Heading 1",
            Kind::H2 => "Heading 2",
            Kind::H3 => "Heading 3",
            Kind::Bullet => "Bulleted list",
            Kind::Numbered => "Numbered list",
            Kind::Todo => "To-do list",
            Kind::Toggle => "Toggle list",
            Kind::Quote => "Quote",
            Kind::Callout => "Callout",
            Kind::Code => "Code",
            Kind::Divider => "Divider",
            Kind::Table => "Table",
            Kind::PageLink => "Page",
            Kind::DbLink => "Database",
            Kind::Image => "Image",
            Kind::Bookmark => "Link preview",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Kind::Text => "Just start writing with plain text.",
            Kind::H1 => "Big section heading.",
            Kind::H2 => "Medium section heading.",
            Kind::H3 => "Small section heading.",
            Kind::Bullet => "Create a simple bulleted list.",
            Kind::Numbered => "Create a list with numbering.",
            Kind::Todo => "Track tasks with a to-do list.",
            Kind::Toggle => "Toggles can hide and show content.",
            Kind::Quote => "Capture a quote.",
            Kind::Callout => "Make writing stand out.",
            Kind::Code => "Capture a code snippet.",
            Kind::Divider => "Visually divide blocks.",
            Kind::Table => "Add a simple table.",
            Kind::PageLink => "Embed a sub-page inside this page.",
            Kind::DbLink => "A table or board of pages, like a task list.",
            Kind::Image => "Add a picture from your computer.",
            Kind::Bookmark => "Show a link as a card.",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Kind::Text => "Aa",
            Kind::H1 => "H1",
            Kind::H2 => "H2",
            Kind::H3 => "H3",
            Kind::Bullet => "•",
            Kind::Numbered => "1.",
            Kind::Todo => "☑",
            Kind::Toggle => "▶",
            Kind::Quote => "❝",
            Kind::Callout => "💡",
            Kind::Code => "</>",
            Kind::Divider => "—",
            Kind::Table => "▦",
            Kind::PageLink => "📄",
            Kind::DbLink => "🗃",
            Kind::Image => "🖼",
            Kind::Bookmark => "🔗",
        }
    }

    pub fn keywords(self) -> &'static str {
        match self {
            Kind::Text => "text plain paragraph",
            Kind::H1 => "heading1 h1 title",
            Kind::H2 => "heading2 h2 subtitle",
            Kind::H3 => "heading3 h3",
            Kind::Bullet => "bulleted list bullet ul",
            Kind::Numbered => "numbered list ol number",
            Kind::Todo => "todo to-do checkbox task",
            Kind::Toggle => "toggle collapse details",
            Kind::Quote => "quote blockquote",
            Kind::Callout => "callout note tip info",
            Kind::Code => "code snippet pre",
            Kind::Divider => "divider line hr separator",
            Kind::Table => "table grid",
            Kind::PageLink => "page subpage link",
            Kind::DbLink => "database table board tasks todo kanban",
            Kind::Image => "image picture photo media",
            Kind::Bookmark => "bookmark link url preview web",
        }
    }

    /// Kinds whose text is a single editable field.
    pub fn is_text_like(self) -> bool {
        !matches!(self, Kind::Divider | Kind::Table | Kind::PageLink | Kind::DbLink | Kind::Image | Kind::Bookmark)
    }

    pub fn is_list(self) -> bool {
        matches!(self, Kind::Bullet | Kind::Numbered | Kind::Todo | Kind::Toggle)
    }
}

#[derive(Clone, Debug)]
pub struct Block {
    pub id: u64,
    pub kind: Kind,
    pub text: String,
    pub checked: bool,
    pub indent: u8,
    /// Toggle: hidden body. Code: language. Callout: icon. PageLink: target page id.
    pub extra: String,
    pub open: bool,
    pub table: Vec<Vec<String>>,
    /// Background colour index (0 = none), see `theme::block_bg`.
    pub color: u8,
    /// Bookmark: description. Image: unused.
    pub desc: String,
    /// Image: display width in points (0 = full width).
    pub size: u16,
}

impl Block {
    pub fn new(kind: Kind, text: impl Into<String>) -> Self {
        let mut b = Block {
            id: next_block_id(),
            kind,
            text: text.into(),
            checked: false,
            indent: 0,
            extra: String::new(),
            open: false,
            table: Vec::new(),
            color: 0,
            desc: String::new(),
            size: 0,
        };
        b.fix_kind_data();
        b
    }

    pub fn fix_kind_data(&mut self) {
        match self.kind {
            Kind::Table if self.table.is_empty() => {
                self.table = vec![
                    vec!["Name".into(), "Notes".into()],
                    vec![String::new(), String::new()],
                    vec![String::new(), String::new()],
                ];
            }
            Kind::Callout if self.extra.is_empty() => self.extra = "💡".into(),
            _ => {}
        }
        if !self.kind.is_list() {
            self.indent = 0;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColKind {
    Text,
    Number,
    Checkbox,
    Date,
    Select,
    Multi,
}

impl ColKind {
    pub const ALL: [ColKind; 6] = [ColKind::Text, ColKind::Number, ColKind::Checkbox, ColKind::Date, ColKind::Select, ColKind::Multi];
    pub fn key(self) -> &'static str {
        match self {
            ColKind::Text => "text",
            ColKind::Number => "number",
            ColKind::Checkbox => "checkbox",
            ColKind::Date => "date",
            ColKind::Select => "select",
            ColKind::Multi => "multi",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ColKind::Text => "Text",
            ColKind::Number => "Number",
            ColKind::Checkbox => "Checkbox",
            ColKind::Date => "Date",
            ColKind::Select => "Select",
            ColKind::Multi => "Multi-select",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            ColKind::Text => "≡",
            ColKind::Number => "#",
            ColKind::Checkbox => "☑",
            ColKind::Date => "📅",
            ColKind::Select => "⊙",
            ColKind::Multi => "☰",
        }
    }
    pub fn from_key(k: &str) -> ColKind {
        ColKind::ALL.into_iter().find(|c| c.key() == k).unwrap_or(ColKind::Text)
    }
}

#[derive(Clone, Debug, Hash)]
pub struct Column {
    pub name: String,
    pub kind: ColKind,
    pub options: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum View {
    Table,
    Board,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct Filter {
    /// Column name, or "Name" for the page title.
    pub col: String,
    pub op: String,
    pub value: String,
}

#[derive(Clone, Debug, Hash)]
pub struct Db {
    pub columns: Vec<Column>,
    pub view: View,
    /// Column used to group the board.
    pub group: String,
    pub filters: Vec<Filter>,
    /// (column name or "Name", ascending)
    pub sorts: Vec<(String, bool)>,
}

impl Db {
    pub fn new() -> Self {
        Db {
            columns: vec![
                Column { name: "Done".into(), kind: ColKind::Checkbox, options: vec![] },
                Column { name: "Due".into(), kind: ColKind::Date, options: vec![] },
                Column { name: "Status".into(), kind: ColKind::Select, options: vec!["Not started".into(), "In progress".into(), "Done".into()] },
                Column { name: "Tags".into(), kind: ColKind::Multi, options: vec![] },
            ],
            view: View::Table,
            group: "Status".into(),
            filters: vec![],
            sorts: vec![],
        }
    }

    pub fn empty() -> Self {
        Db { columns: vec![], view: View::Table, group: String::new(), filters: vec![], sorts: vec![] }
    }
    pub fn group_col(&self) -> Option<&Column> {
        self.columns
            .iter()
            .find(|c| c.name == self.group && c.kind == ColKind::Select)
            .or_else(|| self.columns.iter().find(|c| c.kind == ColKind::Select))
    }
}

#[derive(Clone, Debug)]
pub struct Page {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub parent: String,
    pub order: i64,
    pub favorite: bool,
    pub trashed: bool,
    pub cover: u8,
    pub created: u64,
    pub updated: u64,
    pub blocks: Vec<Block>,
    /// Set when this page is a database; its rows are its child pages.
    pub db: Option<Db>,
    /// Values for the parent database's columns.
    pub props: Vec<(String, String)>,
    /// Keep sub-pages inside a folder named like this page.
    pub folder: bool,
    /// Picture cover, relative to the workspace (overrides `cover`).
    pub cover_image: String,
}

impl Page {
    pub fn new(id: String, title: &str, parent: &str) -> Self {
        let now = crate::store::now_secs();
        Page {
            id,
            title: title.to_string(),
            icon: String::new(),
            parent: parent.to_string(),
            order: 0,
            favorite: false,
            trashed: false,
            cover: 0,
            created: now,
            updated: now,
            blocks: vec![Block::new(Kind::Text, "")],
            db: None,
            props: Vec::new(),
            folder: false,
            cover_image: String::new(),
        }
    }

    pub fn prop(&self, name: &str) -> &str {
        self.props.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()).unwrap_or("")
    }

    pub fn set_prop(&mut self, name: &str, value: String) {
        if let Some(p) = self.props.iter_mut().find(|(k, _)| k == name) {
            p.1 = value;
        } else {
            self.props.push((name.to_string(), value));
        }
    }

    pub fn display_title(&self) -> &str {
        if self.title.trim().is_empty() { "Untitled" } else { &self.title }
    }

    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        for (_, v) in &self.props {
            s.push_str(v);
            s.push(' ');
        }
        for b in &self.blocks {
            s.push_str(&b.text);
            s.push(' ');
            s.push_str(&b.extra);
            s.push(' ');
            for r in &b.table {
                for c in r {
                    s.push_str(c);
                    s.push(' ');
                }
            }
        }
        s
    }
}

// ---------------------------------------------------------------- Markdown out

fn esc_fm(s: &str) -> String {
    s.replace('\n', " ")
}

pub fn to_markdown(p: &Page) -> String {
    let mut out = String::with_capacity(1024);
    out.push_str("---\n");
    out.push_str(&format!("id: {}\n", p.id));
    out.push_str(&format!("title: {}\n", esc_fm(&p.title)));
    if !p.icon.is_empty() {
        out.push_str(&format!("icon: {}\n", p.icon));
    }
    if !p.parent.is_empty() {
        out.push_str(&format!("parent: {}\n", p.parent));
    }
    out.push_str(&format!("order: {}\n", p.order));
    if p.favorite {
        out.push_str("favorite: true\n");
    }
    if p.trashed {
        out.push_str("trashed: true\n");
    }
    if p.cover > 0 {
        out.push_str(&format!("cover: {}\n", p.cover));
    }
    if !p.cover_image.is_empty() {
        out.push_str(&format!("cover_image: {}\n", esc_fm(&p.cover_image)));
    }
    if p.folder {
        out.push_str("folder: true\n");
    }
    out.push_str(&format!("created: {}\nupdated: {}\n", p.created, p.updated));
    if let Some(db) = &p.db {
        out.push_str("database: true\n");
        out.push_str(&format!("view: {}\n", if db.view == View::Board { "board" } else { "table" }));
        if !db.group.is_empty() {
            out.push_str(&format!("group: {}\n", esc_fm(&db.group)));
        }
        for f in &db.filters {
            out.push_str(&format!("filter: {}|{}|{}\n", esc_fm(&f.col).replace('|', "/"), f.op, esc_fm(&f.value).replace('|', "/")));
        }
        for (col, asc) in &db.sorts {
            out.push_str(&format!("sort: {}|{}\n", esc_fm(col).replace('|', "/"), if *asc { "asc" } else { "desc" }));
        }
        for c in &db.columns {
            out.push_str(&format!("column: {}|{}|{}\n", esc_fm(&c.name).replace('|', "/"), c.kind.key(), c.options.iter().map(|o| esc_fm(o).replace([',', '|'], " ")).collect::<Vec<_>>().join(",")));
        }
    }
    for (k, v) in &p.props {
        if !v.is_empty() {
            out.push_str(&format!("prop.{}: {}\n", esc_fm(k).replace(':', " "), esc_fm(v)));
        }
    }
    out.push_str("---\n\n");
    out.push_str(&format!("# {}\n\n", p.display_title()));

    let mut prev_list = false;
    for b in &p.blocks {
        let is_list = b.kind.is_list();
        if !(is_list && prev_list) {
            // blank line between blocks, except inside a run of list items
        }
        let ind = "  ".repeat(b.indent as usize);
        if b.color > 0 {
            out.push_str(&format!("<!-- color:{} -->\n", crate::theme::COLOR_NAMES[(b.color as usize).min(crate::theme::COLOR_NAMES.len() - 1)]));
        }
        let text = b.text.replace('\n', "\\\n");
        match b.kind {
            Kind::Text => {
                if b.text.is_empty() {
                    out.push_str("&nbsp;\n");
                } else {
                    out.push_str(&text);
                    out.push('\n');
                }
            }
            Kind::H1 => out.push_str(&format!("## {}\n", text)),
            Kind::H2 => out.push_str(&format!("### {}\n", text)),
            Kind::H3 => out.push_str(&format!("#### {}\n", text)),
            Kind::Bullet => out.push_str(&format!("{ind}- {}\n", text)),
            Kind::Numbered => out.push_str(&format!("{ind}1. {}\n", text)),
            Kind::Todo => out.push_str(&format!(
                "{ind}- [{}] {}\n",
                if b.checked { "x" } else { " " },
                text
            )),
            Kind::Toggle => {
                out.push_str(&format!(
                    "{ind}<details><summary>{}</summary>\n\n",
                    b.text.replace('\n', " ")
                ));
                for l in b.extra.lines() {
                    out.push_str(&ind);
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("\n{ind}</details>\n"));
            }
            Kind::Quote => {
                for l in b.text.split('\n') {
                    out.push_str(&format!("> {}\n", l));
                }
            }
            Kind::Callout => {
                let mut first = true;
                for l in b.text.split('\n') {
                    if first {
                        out.push_str(&format!("> [!{}] {}\n", b.extra, l));
                        first = false;
                    } else {
                        out.push_str(&format!("> {}\n", l));
                    }
                }
            }
            Kind::Code => {
                out.push_str(&format!("```{}\n{}\n```\n", b.extra, b.text));
            }
            Kind::Divider => out.push_str("---\n"),
            Kind::Table => {
                let cols = b.table.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
                for (ri, row) in b.table.iter().enumerate() {
                    out.push('|');
                    for c in 0..cols {
                        let cell = row.get(c).map(|s| s.as_str()).unwrap_or("");
                        out.push(' ');
                        out.push_str(&cell.replace('|', "\\|").replace('\n', " "));
                        out.push_str(" |");
                    }
                    out.push('\n');
                    if ri == 0 {
                        out.push('|');
                        for _ in 0..cols {
                            out.push_str(" --- |");
                        }
                        out.push('\n');
                    }
                }
            }
            Kind::Image => {
                out.push_str(&format!("![{}]({})", b.text.replace([']', '\n'], " "), b.extra.replace(' ', "%20")));
                if b.size > 0 {
                    out.push_str(&format!(" <!-- width: {} -->", b.size));
                }
                out.push('\n');
            }
            Kind::Bookmark => {
                out.push_str(&format!("[🔖 {}]({})", b.text.replace([']', '\n'], " "), b.extra));
                if !b.desc.is_empty() {
                    out.push_str(&format!(" <!-- desc: {} -->", b.desc.replace("-->", "").replace('\n', " ")));
                }
                out.push('\n');
            }
            Kind::PageLink | Kind::DbLink => {
                out.push_str(&format!("[📄 {}](page:{})\n", b.text.replace(']', ""), b.extra));
            }
        }
        out.push('\n');
        prev_list = is_list;
    }
    out
}

// ---------------------------------------------------------------- Markdown in

pub fn from_markdown(src: &str) -> Page {
    let src = src.replace("\r\n", "\n");
    let mut page = Page::new(String::new(), "", "");
    page.blocks.clear();
    let mut body = src.as_str();

    if let Some(rest) = src.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n").or_else(|| rest.strip_suffix("\n---").map(|r| r.len())) {
            let fm = &rest[..end];
            body = rest.get(end + 5..).unwrap_or("");
            for line in fm.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let v = v.trim();
                    match k.trim() {
                        "id" => page.id = v.to_string(),
                        "title" => page.title = v.to_string(),
                        "icon" => page.icon = v.to_string(),
                        "parent" => page.parent = v.to_string(),
                        "order" => page.order = v.parse().unwrap_or(0),
                        "favorite" => page.favorite = v == "true",
                        "trashed" => page.trashed = v == "true",
                        "cover" => page.cover = v.parse().unwrap_or(0),
                        "cover_image" => page.cover_image = v.to_string(),
                        "folder" => page.folder = v == "true",
                        "filter" => {
                            let db = page.db.get_or_insert_with(Db::empty);
                            let mut parts = v.splitn(3, '|');
                            let col = parts.next().unwrap_or("").trim().to_string();
                            let op = parts.next().unwrap_or("is").trim().to_string();
                            let value = parts.next().unwrap_or("").trim().to_string();
                            if !col.is_empty() {
                                db.filters.push(Filter { col, op, value });
                            }
                        }
                        "sort" => {
                            let db = page.db.get_or_insert_with(Db::empty);
                            let mut parts = v.splitn(2, '|');
                            let col = parts.next().unwrap_or("").trim().to_string();
                            let asc = parts.next().unwrap_or("asc").trim() != "desc";
                            if !col.is_empty() {
                                db.sorts.push((col, asc));
                            }
                        }
                        "created" => page.created = v.parse().unwrap_or(0),
                        "updated" => page.updated = v.parse().unwrap_or(0),
                        "database" => {
                            if v == "true" && page.db.is_none() {
                                page.db = Some(Db::empty());
                            }
                        }
                        "view" => {
                            if let Some(db) = &mut page.db {
                                db.view = if v == "board" { View::Board } else { View::Table };
                            }
                        }
                        "group" => {
                            if let Some(db) = &mut page.db {
                                db.group = v.to_string();
                            }
                        }
                        "column" => {
                            let db = page.db.get_or_insert_with(|| Db::empty());
                            let mut parts = v.splitn(3, '|');
                            let name = parts.next().unwrap_or("").trim().to_string();
                            let kind = ColKind::from_key(parts.next().unwrap_or("text").trim());
                            let options = parts.next().unwrap_or("").split(',').map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect();
                            if !name.is_empty() {
                                db.columns.push(Column { name, kind, options });
                            }
                        }
                        k if k.starts_with("prop.") => {
                            page.props.push((k[5..].to_string(), v.to_string()));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    let lines: Vec<&str> = body.split('\n').collect();
    let mut i = 0;
    let mut title_seen = false;
    // (index of the next block, colour) from a `<!-- color:x -->` line.
    let mut pending_color: Option<(usize, u8)> = None;
    while i < lines.len() {
        if let Some((at, col)) = pending_color {
            if page.blocks.len() > at {
                page.blocks[at].color = col;
                pending_color = None;
            }
        }
        let raw = lines[i];
        let line = raw.trim_end();
        i += 1;
        if line.trim().is_empty() {
            continue;
        }
        // Page title line ("# Title"), first one only.
        if !title_seen && line.starts_with("# ") {
            title_seen = true;
            if page.title.is_empty() {
                page.title = line[2..].trim().to_string();
            }
            continue;
        }
        title_seen = true;
        if let Some(rest) = line.trim().strip_prefix("<!-- color:") {
            let name = rest.trim_end_matches("-->").trim();
            let col = crate::theme::COLOR_NAMES.iter().position(|n| *n == name).unwrap_or(0) as u8;
            pending_color = Some((page.blocks.len(), col));
            continue;
        }

        let indent_spaces = raw.len() - raw.trim_start().len();
        let indent = (indent_spaces / 2).min(6) as u8;
        let t = line.trim_start();

        // Fenced code
        if let Some(lang) = t.strip_prefix("```") {
            let mut code = Vec::new();
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            i += 1; // closing fence
            let mut b = Block::new(Kind::Code, code.join("\n"));
            b.extra = lang.trim().to_string();
            page.blocks.push(b);
            continue;
        }
        // Toggle
        if let Some(rest) = t.strip_prefix("<details><summary>") {
            let summary = rest.split("</summary>").next().unwrap_or("").to_string();
            let mut inner = Vec::new();
            while i < lines.len() && lines[i].trim() != "</details>" {
                inner.push(lines[i].get(indent_spaces.min(lines[i].len() - lines[i].trim_start().len())..).unwrap_or(""));
                i += 1;
            }
            i += 1;
            let mut b = Block::new(Kind::Toggle, summary);
            b.indent = indent;
            b.extra = inner.join("\n").trim_matches('\n').to_string();
            page.blocks.push(b);
            continue;
        }
        // Table
        if t.starts_with('|') {
            let mut rows: Vec<Vec<String>> = Vec::new();
            let mut cur = Some(t);
            loop {
                let Some(l) = cur else { break };
                let l = l.trim();
                if !l.starts_with('|') {
                    i -= 1;
                    break;
                }
                let inner = l.trim_start_matches('|').trim_end_matches('|');
                let cells: Vec<String> = split_cells(inner);
                let is_sep = cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'));
                if !is_sep {
                    rows.push(cells);
                }
                if i < lines.len() {
                    cur = Some(lines[i]);
                    i += 1;
                } else {
                    break;
                }
            }
            let mut b = Block::new(Kind::Table, "");
            b.table = rows;
            b.fix_kind_data();
            page.blocks.push(b);
            continue;
        }
        // Quote / callout (merge consecutive "> " lines)
        if t.starts_with('>') {
            let mut parts = vec![t.trim_start_matches('>').strip_prefix(' ').unwrap_or(t.trim_start_matches('>')).to_string()];
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                let l = lines[i].trim_start().trim_start_matches('>');
                parts.push(l.strip_prefix(' ').unwrap_or(l).to_string());
                i += 1;
            }
            let first = parts[0].clone();
            if let Some(rest) = first.strip_prefix("[!") {
                if let Some((icon, text)) = rest.split_once(']') {
                    parts[0] = text.trim_start().to_string();
                    let mut b = Block::new(Kind::Callout, parts.join("\n"));
                    b.extra = icon.to_string();
                    if b.extra.chars().all(|c| c.is_ascii_alphabetic()) {
                        b.extra = "💡".into();
                    }
                    page.blocks.push(b);
                    continue;
                }
            }
            page.blocks.push(Block::new(Kind::Quote, parts.join("\n")));
            continue;
        }
        if t == "---" || t == "***" || t == "___" {
            page.blocks.push(Block::new(Kind::Divider, ""));
            continue;
        }
        if let Some(rest) = t.strip_prefix("![") {
            if let Some((cap, after)) = rest.split_once("](") {
                let (path, tail) = after.split_once(')').unwrap_or((after, ""));
                let mut b = Block::new(Kind::Image, cap.trim());
                b.extra = path.trim().replace("%20", " ");
                if let Some(w) = tail.split("width:").nth(1) {
                    b.size = w.trim().trim_end_matches("-->").trim().parse().unwrap_or(0);
                }
                page.blocks.push(b);
                continue;
            }
        }
        if let Some(rest) = t.strip_prefix("[🔖") {
            if let Some((title, after)) = rest.split_once("](") {
                let (url, tail) = after.split_once(')').unwrap_or((after, ""));
                let mut b = Block::new(Kind::Bookmark, title.trim());
                b.extra = url.trim().to_string();
                if let Some(d) = tail.split("desc:").nth(1) {
                    b.desc = d.trim().trim_end_matches("-->").trim().to_string();
                }
                page.blocks.push(b);
                continue;
            }
        }
        if t == "&nbsp;" {
            page.blocks.push(Block::new(Kind::Text, ""));
            continue;
        }

        // Single-line blocks with possible "\" continuation.
        let mut text = t.to_string();
        while text.ends_with('\\') && i < lines.len() {
            text.pop();
            text.push('\n');
            text.push_str(lines[i].trim_end());
            i += 1;
        }

        let (kind, rest, checked) = classify(&text);
        if kind == Kind::PageLink {
            // [📄 Title](page:id)
            let title = rest
                .trim_start_matches('[')
                .split("](")
                .next()
                .unwrap_or("")
                .trim_start_matches("📄")
                .trim()
                .to_string();
            let id = rest.split("](page:").nth(1).unwrap_or("").trim_end_matches(')').to_string();
            let mut b = Block::new(Kind::PageLink, title);
            b.extra = id;
            page.blocks.push(b);
            continue;
        }
        let mut b = Block::new(kind, rest);
        b.checked = checked;
        if kind.is_list() {
            b.indent = indent;
        }
        page.blocks.push(b);
    }
    if let Some((at, col)) = pending_color {
        if let Some(b) = page.blocks.get_mut(at) {
            b.color = col;
        }
    }
    if page.blocks.is_empty() {
        page.blocks.push(Block::new(Kind::Text, ""));
    }
    page
}


fn split_cells(inner: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            cur.push('|');
            chars.next();
        } else if c == '|' {
            cells.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

fn classify(t: &str) -> (Kind, String, bool) {
    if let Some(r) = t.strip_prefix("#### ") {
        return (Kind::H3, r.into(), false);
    }
    if let Some(r) = t.strip_prefix("### ") {
        return (Kind::H2, r.into(), false);
    }
    if let Some(r) = t.strip_prefix("## ") {
        return (Kind::H1, r.into(), false);
    }
    if let Some(r) = t.strip_prefix("# ") {
        return (Kind::H1, r.into(), false);
    }
    for p in ["- [ ] ", "* [ ] "] {
        if let Some(r) = t.strip_prefix(p) {
            return (Kind::Todo, r.into(), false);
        }
    }
    for p in ["- [x] ", "- [X] ", "* [x] "] {
        if let Some(r) = t.strip_prefix(p) {
            return (Kind::Todo, r.into(), true);
        }
    }
    if t == "- [ ]" {
        return (Kind::Todo, String::new(), false);
    }
    if t == "- [x]" {
        return (Kind::Todo, String::new(), true);
    }
    for p in ["- ", "* ", "+ "] {
        if let Some(r) = t.strip_prefix(p) {
            return (Kind::Bullet, r.into(), false);
        }
    }
    if t == "-" {
        return (Kind::Bullet, String::new(), false);
    }
    // "12. item"
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && t[digits..].starts_with(". ") {
        return (Kind::Numbered, t[digits + 2..].into(), false);
    }
    if t.starts_with("[📄") && t.contains("](page:") {
        return (Kind::PageLink, t.into(), false);
    }
    (Kind::Text, t.into(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut p = Page::new("abc123".into(), "Hello", "");
        p.blocks = vec![
            Block::new(Kind::H1, "Head"),
            Block::new(Kind::Text, "line one\nline two"),
            Block::new(Kind::Bullet, "a"),
            Block::new(Kind::Todo, "task"),
            Block::new(Kind::Quote, "q1\nq2"),
            Block::new(Kind::Callout, "note"),
            Block::new(Kind::Code, "fn main() {}\n// x"),
            Block::new(Kind::Divider, ""),
            Block::new(Kind::Table, ""),
            Block::new(Kind::Toggle, "more"),
            Block::new(Kind::Text, ""),
        ];
        p.blocks[3].checked = true;
        p.blocks[9].extra = "hidden\nstuff".into();
        p.blocks[4].color = 6;
        let mut im = Block::new(Kind::Image, "A cat");
        im.extra = "_assets/my cat.png".into();
        im.size = 320;
        p.blocks.push(im);
        let mut bm = Block::new(Kind::Bookmark, "Example");
        bm.extra = "https://example.com/a?b=1".into();
        bm.desc = "A site".into();
        p.blocks.push(bm);
        p.folder = true;
        p.cover_image = "_assets/c.jpg".into();
        let mut db = Db::new();
        db.view = View::Board;
        db.filters.push(Filter { col: "Priority".into(), op: "is".into(), value: "High".into() });
        db.sorts.push(("Due".into(), false));
        p.db = Some(db);
        p.props = vec![("Due".into(), "2026-11-20".into()), ("Done".into(), "true".into())];
        let md = to_markdown(&p);
        let q = from_markdown(&md);
        assert_eq!(q.id, "abc123");
        assert_eq!(q.title, "Hello");
        let kinds: Vec<Kind> = q.blocks.iter().map(|b| b.kind).collect();
        let want: Vec<Kind> = p.blocks.iter().map(|b| b.kind).collect();
        assert_eq!(kinds, want, "{md}");
        assert_eq!(q.blocks[1].text, "line one\nline two");
        assert!(q.blocks[3].checked);
        assert_eq!(q.blocks[4].text, "q1\nq2");
        assert_eq!(q.blocks[6].text, "fn main() {}\n// x");
        assert_eq!(q.blocks[8].table.len(), 3);
        assert_eq!(q.blocks[9].extra, "hidden\nstuff");
        assert_eq!(q.blocks[4].color, 6);
        assert_eq!(q.blocks[5].color, 0);
        let db = q.db.as_ref().unwrap();
        assert_eq!(db.view, View::Board);
        assert_eq!(db.columns.len(), 4);
        assert_eq!(db.columns[2].options.len(), 3);
        assert_eq!(q.prop("Due"), "2026-11-20");
        assert_eq!(db.filters.len(), 1);
        assert_eq!(db.sorts, vec![("Due".to_string(), false)]);
        assert!(q.folder);
        assert_eq!(q.cover_image, "_assets/c.jpg");
        let im = &q.blocks[11];
        assert_eq!((im.kind, im.extra.as_str(), im.size, im.text.as_str()), (Kind::Image, "_assets/my cat.png", 320, "A cat"));
        let bm = &q.blocks[12];
        assert_eq!((bm.kind, bm.extra.as_str(), bm.desc.as_str()), (Kind::Bookmark, "https://example.com/a?b=1", "A site"));
    }
}
