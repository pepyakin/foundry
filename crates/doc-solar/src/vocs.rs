//! Vocs site file generation.
//!
//! Generates `vocs.config.ts`, `pages/index.mdx`, `package.json`, and `.gitignore`
//! from the emitted MDX pages.

use foundry_config::DocConfig;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Write all vocs site scaffolding into `out_dir`.
///
/// `pages` is the list of relative MDX paths emitted by the render step (relative to
/// `out_dir/pages/`). `root` is the project root, used to find README files.
pub fn write_site_files(
    out_dir: &Path,
    config: &DocConfig,
    pages: &[PathBuf],
    root: &Path,
) -> eyre::Result<()> {
    fs::create_dir_all(out_dir)?;

    fs::write(out_dir.join(".gitignore"), "dist/\ncache/\nnode_modules/\n")?;
    fs::write(out_dir.join("package.json"), package_json())?;
    fs::write(out_dir.join("vocs.config.ts"), vocs_config(config, pages))?;

    // Homepage: config.homepage → <sources>/README.md → <root>/README.md → empty.
    let homepage_content = find_homepage(config, root);
    let index_path = out_dir.join("pages").join("index.mdx");
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(index_path, homepage_content)?;

    Ok(())
}

// ── vocs.config.ts ────────────────────────────────────────────────────────────

fn vocs_config(config: &DocConfig, pages: &[PathBuf]) -> String {
    let title = if config.title.is_empty() { "Documentation" } else { &config.title };

    let sidebar = build_sidebar(pages);

    let mut ts = String::new();
    ts.push_str("import { defineConfig } from 'vocs'\n\n");
    ts.push_str("export default defineConfig({\n");
    // The config sits next to `pages/` rather than at a project root with a
    // `docs/` subdirectory (vocs' default), so override `rootDir` accordingly.
    ts.push_str("  rootDir: '.',\n");
    ts.push_str(&format!("  title: {},\n", json_str(title)));

    if let Some(repo) = &config.repository {
        ts.push_str(&format!(
            "  editLink: {{ pattern: '{}/edit/main/{{path}}' }},\n",
            repo.trim_end_matches('/')
        ));
    }

    ts.push_str("  sidebar: [\n");
    ts.push_str(&sidebar);
    ts.push_str("  ],\n");
    ts.push_str("})\n");
    ts
}

/// Build a TypeScript sidebar array string from the list of relative page paths.
fn build_sidebar(pages: &[PathBuf]) -> String {
    // Sort pages for deterministic output.
    let mut sorted = pages.to_vec();
    sorted.sort();

    // Group by parent directory.
    let mut groups: std::collections::BTreeMap<String, Vec<(String, String)>> = Default::default();

    for page in &sorted {
        let link = format!("/{}", page.with_extension("").display());
        let text = page.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
        let dir = page
            .parent()
            .and_then(|p| if p == Path::new("") { None } else { p.to_str() })
            .unwrap_or("")
            .to_string();
        groups.entry(dir).or_default().push((text, link));
    }

    let mut out = String::new();
    for (dir, items) in &groups {
        if dir.is_empty() {
            // Top-level items (shouldn't normally happen since we put everything in subdirs).
            for (text, link) in items {
                out.push_str(&format!(
                    "    {{ text: {}, link: {} }},\n",
                    json_str(text),
                    json_str(link)
                ));
            }
        } else {
            // Directory group.
            let group_name = dir_display_name(dir);
            out.push_str(&format!(
                "    {{\n      text: {},\n      items: [\n",
                json_str(&group_name)
            ));
            for (text, link) in items {
                out.push_str(&format!(
                    "        {{ text: {}, link: {} }},\n",
                    json_str(text),
                    json_str(link)
                ));
            }
            out.push_str("      ],\n    },\n");
        }
    }
    out
}

/// Turn a directory path like `src/contracts` into `src/contracts`.
fn dir_display_name(dir: &str) -> String {
    dir.to_string()
}

// ── homepage ──────────────────────────────────────────────────────────────────

fn find_homepage(config: &DocConfig, root: &Path) -> String {
    // 1. Explicit homepage from config.
    if let Some(hp) = &config.homepage {
        let path = if hp.is_absolute() { hp.clone() } else { root.join(hp) };
        if let Ok(content) = fs::read_to_string(&path) {
            return content;
        }
    }

    // 2. <root>/README.md
    let readme = root.join("README.md");
    if let Ok(content) = fs::read_to_string(&readme) {
        return content;
    }

    // 3. Empty fallback.
    String::new()
}

// ── package.json ──────────────────────────────────────────────────────────────

const fn package_json() -> &'static str {
    r#"{
  "devDependencies": {
    "vocs": "^1"
  },
  "scripts": {
    "dev": "vocs dev",
    "build": "vocs build"
  }
}
"#
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn json_str(s: &str) -> String {
    // Escape double quotes and wrap.
    format!("'{}'", s.replace('\'', "\\'"))
}
