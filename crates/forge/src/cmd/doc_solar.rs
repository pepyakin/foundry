//! `forge doc-solar`
//!
//! Solar-powered, vocs-targeted documentation generator. Eventually
//! supersedes `forge doc`.

use crate::cmd::watch::WatchArgs;
use clap::{Parser, ValueHint};
use eyre::{Result, WrapErr};
use forge_doc_solar::DocBuilder;
use foundry_cli::{opts::GH_REPO_PREFIX_REGEX, utils::Git};
use foundry_common::compile::ProjectCompiler;
use foundry_config::{Config, load_config_with_root};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Clone, Debug, Parser)]
pub struct DocSolarArgs {
    /// The project's root path.
    ///
    /// By default root of the Git repository, if in one,
    /// or the current working directory.
    #[arg(long, value_hint = ValueHint::DirPath, value_name = "PATH")]
    pub root: Option<PathBuf>,

    /// The doc's output path.
    ///
    /// By default, it is the `docs/` in project root.
    #[arg(long, short, value_hint = ValueHint::DirPath, value_name = "PATH")]
    out: Option<PathBuf>,

    /// Build the vocs site from generated MDX files.
    #[arg(long, short)]
    build: bool,

    /// Serve the documentation (runs `npx vocs dev`).
    #[arg(long, short)]
    serve: bool,

    /// Open the documentation in a browser after serving.
    #[arg(long, requires = "serve")]
    open: bool,

    /// Hostname for serving documentation.
    #[arg(long, requires = "serve")]
    hostname: Option<String>,

    #[command(flatten)]
    pub watch: WatchArgs,

    /// Port for serving documentation.
    #[arg(long, short, requires = "serve")]
    port: Option<usize>,

    /// The relative path to the `hardhat-deploy` or `forge-deploy` artifact directory.
    /// Leave blank for default (`<root>/deployments`).
    #[arg(long)]
    deployments: Option<Option<PathBuf>>,

    /// Whether to create docs for external libraries.
    #[arg(long, short)]
    include_libraries: bool,
}

impl DocSolarArgs {
    pub async fn run(self) -> Result<()> {
        let config = self.config()?;
        let root = &config.root;
        let project = config.project()?;

        // Reuse the same `solar::sema::Compiler` instance the rest of forge uses.
        let mut output = ProjectCompiler::new().quiet(true).compile(&project)?;
        let compiler = output.parser_mut().solc_mut().compiler_mut();

        let mut doc_cfg = config.doc;
        if let Some(out) = self.out.clone() {
            doc_cfg.out = out;
        }

        // Auto-detect repository URL from git remote.
        if doc_cfg.repository.is_none()
            && let Some(remote) = Git::new(root).remote_url("origin")
            && let Some(captures) = GH_REPO_PREFIX_REGEX.captures(&remote)
        {
            let brand = captures.name("brand").unwrap().as_str();
            let tld = captures.name("tld").unwrap().as_str();
            let project_path = GH_REPO_PREFIX_REGEX.replace(&remote, "");
            doc_cfg.repository =
                Some(format!("https://{brand}.{tld}/{}", project_path.trim_end_matches(".git")));
        }

        // Step 5: detect HEAD commit hash for Git Source links.
        let commit = Git::new(root).commit_hash(false, "HEAD").ok();

        let mut builder = DocBuilder::new(
            root.clone(),
            project.paths.sources,
            project.paths.libraries,
            self.include_libraries,
        );
        builder.should_build = self.build;
        builder.commit = commit;
        builder.deployments = self.deployments.clone();
        builder.config = doc_cfg.clone();

        let out_dir = if doc_cfg.out.is_absolute() { doc_cfg.out } else { root.join(&doc_cfg.out) };

        builder.build(compiler)?;

        if self.serve {
            run_vocs_dev(
                &out_dir,
                self.hostname.as_deref().unwrap_or("localhost"),
                self.port.unwrap_or(3000),
                self.open,
            )?;
        }

        Ok(())
    }

    /// Returns whether watch mode is enabled.
    pub const fn is_watch(&self) -> bool {
        self.watch.watch.is_some()
    }

    pub fn config(&self) -> Result<Config> {
        load_config_with_root(self.root.as_deref())
    }
}

/// Step 7: shell out to `npx vocs dev` against the generated site.
fn run_vocs_dev(out_dir: &std::path::Path, hostname: &str, port: usize, open: bool) -> Result<()> {
    sh_println!("Serving on: http://{hostname}:{port}")?;
    let mut cmd = Command::new("npx");
    cmd.arg("--yes")
        .arg("vocs")
        .arg("dev")
        .arg("--host")
        .arg(hostname)
        .arg("--port")
        .arg(port.to_string())
        .current_dir(out_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if open {
        cmd.arg("--open");
    }
    let status = cmd.status().wrap_err(
        "failed to spawn `npx vocs dev`; ensure Node.js / npm is installed and on PATH",
    )?;
    if !status.success() {
        eyre::bail!("`npx vocs dev` exited with status {status}");
    }
    Ok(())
}
