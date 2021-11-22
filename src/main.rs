use console::style;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use structopt::StructOpt;
use substring::Substring;

// TODO use ColoredHelp by default?
#[derive(StructOpt, Debug)]
enum Command {
    /// Adds a mod to the current instance
    #[structopt(setting = structopt::clap::AppSettings::ColoredHelp)]
    Add { package_name: String },
    /// Removes a mod
    #[structopt(setting = structopt::clap::AppSettings::ColoredHelp)]
    Remove { package_name: String },
    #[structopt(setting = structopt::clap::AppSettings::ColoredHelp)]
    Get { package_name: String },
    #[structopt(setting = structopt::clap::AppSettings::ColoredHelp)]
    Update,
    #[structopt(setting = structopt::clap::AppSettings::ColoredHelp)]
    Clean,
}

#[derive(StructOpt, Debug)]
#[structopt(name = "hopper", setting = structopt::clap::AppSettings::ColoredHelp)]
struct Args {
    /// Path to configuration file
    #[structopt(short, long, parse(from_os_str))]
    config: Option<PathBuf>,

    /// Path to mod lockfile
    #[structopt(short, long, parse(from_os_str))]
    lockfile: Option<PathBuf>,

    #[structopt(subcommand)]
    command: Command,
}

impl Args {
    fn load_config(&self) -> Result<Config, confy::ConfyError> {
        if let Some(config_path) = &self.config {
            confy::load_path(config_path)
        } else {
            confy::load("hopper")
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct Upstream {
    /// Modrinth main server address
    server_address: String,
}

impl Default for Upstream {
    fn default() -> Self {
        Self {
            server_address: "api.modrinth.com".into(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct Options {
    /// Whether to reverse search results
    reverse_search: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            reverse_search: true,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Default)]
struct Config {
    /// General settings
    options: Options,

    /// Configuration for the upstream Modrinth server
    upstream: Upstream,
}

#[derive(Deserialize, Debug)]
struct SearchResponse {
    hits: Vec<ModResult>,
    offset: isize,
    limit: isize,
    total_hits: isize,
}

#[derive(Deserialize, Debug)]
struct ModResult {
    mod_id: String,               // TODO parse to `local-xxxxx` with regex
    project_type: Option<String>, // NOTE this isn't in all search results?
    author: String,
    title: String,
    description: String,
    categories: Vec<String>,
    versions: Vec<String>,
    downloads: isize,
    page_url: String,
    icon_url: String,
    author_url: String,
    date_created: String,
    date_modified: String,
    latest_version: String,
    license: String,
    client_side: String,
    server_side: String,
    host: String,
}

#[derive(Deserialize, Debug)]
struct ModInfo {
    id: String, // TODO serialize mod id?
    slug: String,
    team: String, // TODO serialize team id?
    title: String,
    description: String,
    body: String,
    published: String, // TODO serialize datetime
    updated: String,   // TODO serialize datetime
    status: String,
    // TODO License object
    // license: String,
    client_side: String, // TODO serialize as enum
    server_side: String, // TODO serialize as enum
    downloads: isize,
    followers: isize,
    categories: Vec<String>,
    versions: Vec<String>,
    icon_url: Option<String>,
    issues_url: Option<String>,
    source_url: Option<String>,
    wiki_url: Option<String>,
    discord_url: Option<String>,
    donation_urls: Vec<String>,
}

impl ModResult {
    fn format_info(&self) -> String {
        let title = style(self.title.clone()).bold();
        let downloads = style(self.downloads.clone()).bold().green();
        if let Some(latest_release) = self.versions.last() {
            // TODO fetch version numbers to display
            let latest_release = style(latest_release).bold().blue();
            format!("{} [{}] ({} downloads)", title, latest_release, downloads)
        } else {
            format!("{} [no releases]", title)
        }
    }

    fn format_description(&self) -> String {
        self.description.to_owned()
    }

    fn display(&self, index: usize) {
        let index = style(index).magenta();
        let info = self.format_info();
        let description = self.format_description();
        println!("{} {}\n    {}", index, info, description);
    }
}

async fn search_mods(config: &Config, query: String) -> anyhow::Result<SearchResponse> {
    let client = reqwest::Client::new();
    let url = format!("https://{}/api/v1/mod", config.upstream.server_address);
    let params = [("query", query.as_str())];
    let url = reqwest::Url::parse_with_params(url.as_str(), &params)?;
    let response = client
        .get(url)
        .send()
        .await?
        .json::<SearchResponse>()
        .await?;
    Ok(response)
}

// TODO config flag to reverse search results order
fn display_search_results(config: &Config, response: &SearchResponse) {
    let iter = response.hits.iter().enumerate();
    if config.options.reverse_search {
        for (i, result) in iter.rev() {
            result.display(i + 1);
        }
    } else {
        for (i, result) in iter {
            result.display(i + 1);
        }
    }
}

// TODO implement enum for more graceful exiting
async fn select_from_results<'a>(
    _config: &Config,
    response: &'a SearchResponse,
) -> Vec<&'a ModResult> {
    // TODO actually select with a dialogue
    match response.hits.first() {
        Some(first) => vec![first],
        None => Vec::new(),
    }
}

async fn fetch_mod_info(config: &Config, mod_result: &ModResult) -> anyhow::Result<ModInfo> {
    let client = reqwest::Client::new();
    let mod_id = &mod_result.mod_id;
    let mod_id = mod_id.substring(6, mod_id.len()); // Remove "local-" prefix
    let url = format!(
        "https://{}/api/v1/mod/{}",
        config.upstream.server_address, mod_id
    );
    let response = client.get(url).send().await?;
    let response = response.json::<ModInfo>().await?;
    Ok(response)
}

async fn cmd_get(config: &Config, package_name: String) -> anyhow::Result<()> {
    let response = search_mods(config, package_name).await?;

    if response.hits.is_empty() {
        // TODO formatting
        println!("No results; nothing to do...");
        return Ok(());
    }

    display_search_results(config, &response);
    let selected = select_from_results(config, &response).await;

    if selected.is_empty() {
        // TODO formatting
        println!("No packages selected; nothing to do...");
        return Ok(());
    }

    for to_get in selected.iter() {
        let mod_info = fetch_mod_info(config, to_get).await?;
        println!("mod: {:#?}", mod_info);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::from_args();
    let config = args.load_config()?;
    match args.command {
        Command::Get { package_name } => cmd_get(&config, package_name).await,
        _ => unimplemented!("unimplemented subcommand"),
    }
}
