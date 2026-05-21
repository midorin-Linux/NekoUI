use anyhow::{Context, Result, bail};
use colored::Colorize;
use dialoguer::{Confirm, Input, Password, Select, theme::SimpleTheme};
use nekoai_config::loader::{
    ChatPlatform, Config, ConversationModel, DEFAULT_QDRANT_URL, Discord, EmbeddingModel, Memory,
    Parameters, Provider, SearxngConfig, SecretKey, SummarizerModel, ToolPermissions, VectorDb,
    WebUiConfig,
};

// ── Provider Presets ──────────────────────────────────────────────────────────

struct ProviderPreset {
    label: &'static str,
    base_url: &'static str,
    default_model: &'static str,
    default_summarizer: &'static str,
}

const PROVIDER_PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        label: "OpenAI",
        base_url: "https://api.openai.com/v1",
        default_model: "gpt-4o",
        default_summarizer: "gpt-4o-mini",
    },
    ProviderPreset {
        label: "Anthropic",
        base_url: "https://api.anthropic.com/v1",
        default_model: "claude-sonnet-4-20250514",
        default_summarizer: "claude-haiku-3-5-20241022",
    },
    ProviderPreset {
        label: "Ollama (Local)",
        base_url: "http://localhost:11434/v1",
        default_model: "llama3.1",
        default_summarizer: "llama3.1",
    },
    ProviderPreset {
        label: "Custom",
        base_url: "",
        default_model: "",
        default_summarizer: "",
    },
];

// ── Embedding Presets ─────────────────────────────────────────────────────────

struct EmbeddingPreset {
    label: &'static str,
    model_name: &'static str,
    dimension: u64,
}

const EMBEDDING_PRESETS: &[EmbeddingPreset] = &[
    EmbeddingPreset {
        label: "text-embedding-3-small (OpenAI, 1536d)",
        model_name: "text-embedding-3-small",
        dimension: 1536,
    },
    EmbeddingPreset {
        label: "text-embedding-3-large (OpenAI, 3072d)",
        model_name: "text-embedding-3-large",
        dimension: 3072,
    },
    EmbeddingPreset {
        label: "text-embedding-ada-002 (OpenAI, 1536d)",
        model_name: "text-embedding-ada-002",
        dimension: 1536,
    },
    EmbeddingPreset {
        label: "Custom",
        model_name: "",
        dimension: 0,
    },
];

// ── Validation & Input Helpers ───────────────────────────────────────────────

/// Prompt for input with validation in a loop until valid input is provided.
fn validated_input<V>(prompt: &str, default: Option<String>, validate: V) -> Result<String>
where
    V: Fn(&str) -> Result<(), String>,
{
    loop {
        let mut input_builder = Input::with_theme(&SimpleTheme).with_prompt(prompt);
        if let Some(ref default_val) = default {
            input_builder = input_builder.with_initial_text(default_val.clone());
        }
        let value: String = input_builder.interact_text()?;
        match validate(&value) {
            Ok(()) => return Ok(value),
            Err(msg) => {
                println!("  {} {}\n", "✗".red(), msg.red());
            }
        }
    }
}

/// Prompt for a password with validation in a loop.
fn validated_password<V>(prompt: &str, validate: V) -> Result<String>
where
    V: Fn(&str) -> Result<(), String>,
{
    loop {
        let value = Password::with_theme(&SimpleTheme)
            .with_prompt(prompt)
            .interact()?;
        match validate(&value) {
            Ok(()) => return Ok(value),
            Err(msg) => {
                println!("  {} {}\n", "✗".red(), msg.red());
            }
        }
    }
}

// ── Validation Functions ──────────────────────────────────────────────────────

fn validate_discord_token(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("Token cannot be empty".to_string());
    }
    if s.len() < 50 {
        return Err(
            "Token is too short — Discord bot tokens are typically 50+ characters".to_string(),
        );
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(
            "Token contains invalid characters — only alphanumeric, '.', '_', and '-' are allowed"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_url(s: &str) -> Result<(), String> {
    if !s.starts_with("http://") && !s.starts_with("https://") {
        return Err("URL must start with http:// or https://".to_string());
    }
    if s.len() < 10 {
        return Err("URL is too short".to_string());
    }
    Ok(())
}

fn validate_api_key(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("API Key cannot be empty".to_string());
    }
    Ok(())
}

fn validate_guild_id(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Ok(()); // empty means use default (0)
    }
    match s.parse::<u64>() {
        Ok(_) => Ok(()),
        Err(_) => Err("Guild ID must be a number (e.g., 123456789012345678)".to_string()),
    }
}

fn validate_dimension(s: &str) -> Result<(), String> {
    match s.parse::<u64>() {
        Ok(n) if n > 0 => Ok(()),
        Ok(_) => Err("Dimension must be a positive number".to_string()),
        Err(_) => Err("Dimension must be a positive number (e.g., 1536)".to_string()),
    }
}

fn validate_positive_number(s: &str) -> Result<(), String> {
    match s.parse::<u64>() {
        Ok(n) if n > 0 => Ok(()),
        Ok(_) => Err("Value must be a positive number".to_string()),
        Err(_) => Err("Value must be a positive number".to_string()),
    }
}

fn validate_temperature(s: &str) -> Result<(), String> {
    match s.parse::<f64>() {
        Ok(n) if (0.0 ..= 2.0).contains(&n) => Ok(()),
        Ok(_) => Err("Temperature must be between 0.0 and 2.0".to_string()),
        Err(_) => Err("Temperature must be a decimal number (e.g., 1.0)".to_string()),
    }
}

fn validate_top_p(s: &str) -> Result<(), String> {
    match s.parse::<f64>() {
        Ok(n) if (0.0 ..= 1.0).contains(&n) => Ok(()),
        Ok(_) => Err("Top P must be between 0.0 and 1.0".to_string()),
        Err(_) => Err("Top P must be a decimal number (e.g., 0.95)".to_string()),
    }
}

/// Prompt for a full set of model parameters (max_token, temperature, top_p)
/// using the given `defaults` and displaying the section `label`.
fn input_model_params(label: &str, defaults: &Parameters) -> Result<Parameters> {
    let max_token_str = validated_input(
        &format!("  Max tokens ({label})"),
        Some(defaults.max_token.to_string()),
        validate_positive_number,
    )?;
    let max_token: u64 = max_token_str.parse().unwrap_or(defaults.max_token);

    println!();

    let temperature = validated_input(
        &format!("  Temperature (0.0 ~ 2.0; higher = more creative) ({label})"),
        Some(defaults.temperature.to_string()),
        validate_temperature,
    )?;
    let temperature: f64 = temperature.parse().unwrap_or(defaults.temperature);

    println!();

    let top_p = validated_input(
        &format!("  Top P (0.0 ~ 1.0; nucleus sampling) ({label})"),
        Some(defaults.top_p.to_string()),
        validate_top_p,
    )?;
    let top_p: f64 = top_p.parse().unwrap_or(defaults.top_p);

    println!();

    Ok(Parameters {
        max_token,
        temperature,
        top_p,
    })
}

// ── Display Helpers ───────────────────────────────────────────────────────────

fn print_header(step: usize, total: usize, title: &str) {
    println!();
    println!(
        "  {}",
        format!("═══ Step {}/{} : {} ", step, total, title)
            .cyan()
            .bold()
    );
    println!();
}

fn print_footer() {
    println!();
    println!("  {}", "[Enter: next]  [Esc/Ctrl+C: exit]".dimmed());
}

fn print_subheader(text: &str) {
    println!("  {}", text.dimmed());
    println!();
}

// ── Step 1: Discord ──────────────────────────────────────────────────────────

fn step_discord() -> Result<(String, u64)> {
    print_header(1, 5, "Discord Bot Configuration");
    println!("  Enter your Discord Bot Token and Guild ID.");
    print_subheader("Create a Bot on the Discord Developer Portal and invite it to your server.");

    let token = validated_password("  Bot Token", validate_discord_token)?;
    println!();

    let guild_id_str = validated_input(
        "  Guild / Server ID (optional — leave empty for single-server mode)",
        None,
        validate_guild_id,
    )?;

    let guild_id: u64 = guild_id_str.parse().unwrap_or(0);
    print_footer();
    Ok((token, guild_id))
}

// ── Step 2: AI Provider ──────────────────────────────────────────────────────

fn step_provider() -> Result<(String, String, String, String)> {
    print_header(2, 5, "AI Provider");
    println!("  Select your AI provider and enter the API credentials.");
    print_subheader(
        "The provider handles all model inference including conversation, summarization, and embedding.",
    );

    let provider_labels: Vec<&str> = PROVIDER_PRESETS.iter().map(|p| p.label).collect();
    let selection = Select::with_theme(&SimpleTheme)
        .with_prompt("  Provider")
        .items(&provider_labels)
        .default(0)
        .interact()?;

    let preset = &PROVIDER_PRESETS[selection];
    println!();

    let base_url = validated_input(
        "  Base URL",
        Some(preset.base_url.to_string()),
        validate_url,
    )?;

    println!();

    let api_key = validated_password("  API Key", validate_api_key)?;

    print_footer();
    Ok((
        preset.default_model.to_string(),
        preset.default_summarizer.to_string(),
        base_url,
        api_key,
    ))
}

// ── Step 3: Model Selection ──────────────────────────────────────────────────

fn step_models(
    default_model: &str,
    default_summarizer: &str,
) -> Result<(String, String, String, u64)> {
    print_header(3, 5, "Model Selection");
    println!("  Choose the AI models for conversation, summarization, and embedding.");
    println!(
        "  {}",
        "  The conversation model handles user interactions. The summarizer model (can be a cheaper/faster one) handles memory summarization."
            .dimmed()
    );
    println!();

    let default_model_display = if default_model.is_empty() {
        "gpt-4o".to_string()
    } else {
        default_model.to_string()
    };

    let model_name = validated_input(
        "  Conversation model name",
        Some(default_model_display),
        |_| Ok(()),
    )?;

    println!();

    let default_summarizer_display = if default_summarizer.is_empty() {
        model_name.clone()
    } else {
        default_summarizer.to_string()
    };

    let summarizer_input = validated_input(
        "  Summarizer model name (press Enter to use the same as the conversation model)",
        Some(default_summarizer_display),
        |_| Ok(()),
    )?;

    let summarizer_model_name = if summarizer_input.is_empty() {
        model_name.clone()
    } else {
        summarizer_input
    };

    println!();
    println!("  Select an embedding model for vector search (long-term & mid-term memory):");
    println!();

    let embed_labels: Vec<&str> = EMBEDDING_PRESETS.iter().map(|p| p.label).collect();
    let embed_idx = Select::with_theme(&SimpleTheme)
        .with_prompt("  Embedding model")
        .items(&embed_labels)
        .default(0)
        .interact()?;

    let (embed_model_name, embed_dimension) = {
        let preset = &EMBEDDING_PRESETS[embed_idx];
        if preset.model_name.is_empty() {
            // "Custom" option selected
            println!();
            let name = validated_input("  Embedding model name", None, |_| Ok(()))?;
            let dim = validated_input(
                "  Embedding dimension",
                Some("1536".to_string()),
                validate_dimension,
            )?;
            let dim: u64 = dim
                .parse()
                .context("Invalid dimension — expected a positive integer")?;
            (name, dim)
        } else {
            (preset.model_name.to_string(), preset.dimension)
        }
    };

    print_footer();
    Ok((
        model_name,
        summarizer_model_name,
        embed_model_name,
        embed_dimension,
    ))
}

// ── Step 4: Tool Permissions ─────────────────────────────────────────────────

fn step_tools() -> Result<ToolPermissions> {
    print_header(4, 5, "Tool Permissions");
    println!("  Enable the tools you want the agent to use:");
    print_subheader("You can change these later by editing the config file.");

    let web_search = Confirm::with_theme(&SimpleTheme)
        .with_prompt("  Enable Web Search?")
        .default(false)
        .interact()?;

    let searxng = if web_search {
        println!();
        println!("  {}", "─── SearxNG Settings ───".bold());
        println!();

        let defaults = SearxngConfig::default();
        let base_url = validated_input(
            "  SearxNG URL",
            Some(defaults.base_url.clone()),
            validate_url,
        )?;

        println!();

        let max_results_str = validated_input(
            "  SearxNG max results",
            Some(defaults.max_results.to_string()),
            validate_positive_number,
        )?;
        let max_results: u64 = max_results_str.parse().unwrap_or(defaults.max_results);

        SearxngConfig {
            base_url,
            max_results,
        }
    } else {
        Default::default()
    };

    print_footer();
    Ok(ToolPermissions {
        web_search,
        searxng,
        code_exec: false,
        read_file: false,
        code_exec_sandbox: Default::default(),
        read_file_dirs: Default::default(),
    })
}

// ── Advanced Settings Data ───────────────────────────────────────────────────

struct AdvancedConfig {
    memory: Memory,
    params: Parameters,
    summarizer_params: Parameters,
}

// ── Step 5: Advanced Settings (Optional) ─────────────────────────────────────

fn step_advanced() -> Result<AdvancedConfig> {
    print_header(5, 5, "Advanced Settings");

    let configure = Confirm::with_theme(&SimpleTheme)
        .with_prompt("  Do you want to configure memory and model parameters?")
        .default(false)
        .interact()?;

    if !configure {
        println!(
            "  {}",
            "(Using default values for all advanced settings)".dimmed()
        );
        print_footer();
        return Ok(AdvancedConfig {
            memory: Memory::default(),
            params: Parameters::default(),
            summarizer_params: Parameters::default(),
        });
    }

    println!();
    println!("  {}", "─── Memory Settings ───".bold());
    println!();
    println!(
        "  {}",
        "If you are using Docker Compose, start Qdrant with: docker compose up -d qdrant"
            .to_string()
            .dimmed()
    );
    println!();

    let qdrant_url = validated_input(
        "  Qdrant URL",
        Some(DEFAULT_QDRANT_URL.to_string()),
        validate_url,
    )?;

    println!();

    let qdrant_api_key: String = Input::with_theme(&SimpleTheme)
        .with_prompt("  Qdrant API Key (leave empty if not required)")
        .allow_empty(true)
        .interact_text()?;

    let qdrant_api_key = if qdrant_api_key.is_empty() {
        None
    } else {
        Some(qdrant_api_key)
    };

    println!();

    let short_term_max = validated_input(
        "  Short-term memory max entries",
        Some("20".to_string()),
        validate_positive_number,
    )?;
    let short_term_max_entries: usize = short_term_max.parse().unwrap_or(20);

    println!();

    let mid_term_top_k_str = validated_input(
        "  Mid-term memory top-K results",
        Some("3".to_string()),
        validate_positive_number,
    )?;
    let mid_term_top_k: usize = mid_term_top_k_str.parse().unwrap_or(3);

    println!();

    let long_term_top_k_str = validated_input(
        "  Long-term memory top-K results",
        Some("5".to_string()),
        validate_positive_number,
    )?;
    let long_term_top_k: usize = long_term_top_k_str.parse().unwrap_or(5);

    println!();

    let retention_days_str = validated_input(
        "  Mid-term memory retention (days)",
        Some("30".to_string()),
        validate_positive_number,
    )?;
    let mid_term_retention_days: u32 = retention_days_str.parse().unwrap_or(30);

    println!();

    let long_term_interval_str = validated_input(
        "  Long-term memory extraction interval (messages)",
        Some("10".to_string()),
        validate_positive_number,
    )?;
    let long_term_extraction_interval: usize = long_term_interval_str.parse().unwrap_or(10);

    println!();
    println!("  {}", "─── Model Parameters ───".bold());
    println!();
    println!(
        "  {}",
        "These settings affect the behavior of the conversation and summarizer models.".dimmed()
    );
    println!();

    let params = input_model_params("conversation model", &Parameters::default())?;

    println!("  {}", "─── Summarizer Model Parameters ───".bold());
    println!();

    let summarizer_params = input_model_params("summarizer model", &Parameters::default())?;

    let memory = Memory {
        vector_db: VectorDb {
            url: qdrant_url,
            api_key: qdrant_api_key,
            ..Default::default()
        },
        short_term_max_entries,
        mid_term_top_k,
        long_term_top_k,
        mid_term_retention_days,
        long_term_extraction_interval,
    };

    print_footer();
    Ok(AdvancedConfig {
        memory,
        params,
        summarizer_params,
    })
}

// ── Summary ──────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn print_summary(
    token_len: usize,
    _base_url: &str,
    model_name: &str,
    summarizer_model_name: &str,
    embed_model_name: &str,
    embed_dimension: u64,
    tools: &ToolPermissions,
    guild_id: u64,
    advanced: &AdvancedConfig,
) {
    println!();
    println!("  {}", "═══ Configuration Summary ═══".green().bold());
    println!();

    println!("  Discord Token  : {}", "*".repeat(4.min(token_len)));
    if guild_id != 0 {
        println!("  Guild ID        : {}", guild_id);
    } else {
        println!(
            "  Guild ID        : {}",
            "(not set / single-server mode)".dimmed()
        );
    }
    println!("  Base URL        : {}", "(redacted)".dimmed());
    println!("  Conversation    : {}", model_name);
    println!("  Summarizer      : {}", summarizer_model_name);
    println!(
        "  Embedding Model : {} (dim: {})",
        embed_model_name, embed_dimension
    );
    println!(
        "  Web Search      : {}",
        if tools.web_search {
            "✓ enabled".green()
        } else {
            "✗ disabled".red()
        }
    );
    if tools.web_search {
        println!();
        println!("  {}", "─── SearxNG ───".dimmed());
        println!("  URL             : {}", tools.searxng.base_url);
        println!("  Max results     : {}", tools.searxng.max_results);
    }
    // Advanced settings
    let m = &advanced.memory;
    println!();
    println!("  {}", "─── Memory ───".dimmed());
    println!("  Qdrant URL      : {}", m.vector_db.url);
    println!("  Short-term max  : {}", m.short_term_max_entries);
    println!("  Mid-term top-K  : {}", m.mid_term_top_k);
    println!("  Long-term top-K : {}", m.long_term_top_k);
    println!("  Retention (days): {}", m.mid_term_retention_days);

    let p = &advanced.params;
    println!();
    println!("  {}", "─── Conversation Parameters ───".dimmed());
    println!("  Max tokens      : {}", p.max_token);
    println!("  Temperature     : {:.2}", p.temperature);
    println!("  Top P           : {:.2}", p.top_p);

    let sp = &advanced.summarizer_params;
    println!("  {}", "─── Summarizer Parameters ───".dimmed());
    println!("  Max tokens      : {}", sp.max_token);
    println!("  Temperature     : {:.2}", sp.temperature);
    println!("  Top P           : {:.2}", sp.top_p);
    println!();
}

// ── Main Wizard Orchestrator ─────────────────────────────────────────────────

/// Run all 5 steps of the setup wizard and return a complete Config.
pub fn run_wizard() -> Result<Config> {
    // ── Step 1: Discord ──────────────────────────────────────
    let (token, guild_id) = step_discord()?;

    // ── Step 2: AI Provider ──────────────────────────────────
    let (default_model, default_summarizer, base_url, api_key) = step_provider()?;

    // ── Step 3: Model Selection ──────────────────────────────
    let (model_name, summarizer_model_name, embed_model_name, embed_dimension) =
        step_models(&default_model, &default_summarizer)?;

    // ── Step 4: Tool Permissions ─────────────────────────────
    let tools = step_tools()?;

    // ── Step 5: Advanced Settings ────────────────────────────
    let advanced = step_advanced()?;

    // ── Summary & Confirm ────────────────────────────────────
    print_summary(
        token.len(),
        &base_url,
        &model_name,
        &summarizer_model_name,
        &embed_model_name,
        embed_dimension,
        &tools,
        guild_id,
        &advanced,
    );

    let confirmed = Confirm::with_theme(&SimpleTheme)
        .with_prompt("  Save this configuration?")
        .default(true)
        .interact()?;

    if !confirmed {
        bail!("setup cancelled by user");
    }

    // ── Build Config ─────────────────────────────────────────
    // Reorder to minimize clones: fields used last get the move instead of clone
    let config = Config {
        chat_platform: ChatPlatform::Discord,
        discord: Discord {
            token: SecretKey::new(token),
            guild_id,
        },
        provider: Provider {
            conversation_model: ConversationModel {
                provider_base_url: base_url.clone(),
                api_key: SecretKey::new(api_key.clone()),
                model_name,
                parameters: advanced.params,
            },
            summarizer_model: SummarizerModel {
                provider_base_url: base_url.clone(),
                api_key: SecretKey::new(api_key.clone()),
                model_name: summarizer_model_name,
                parameters: advanced.summarizer_params,
            },
            embedding_model: EmbeddingModel {
                provider_base_url: base_url,
                api_key: SecretKey::new(api_key.clone()),
                model_name: embed_model_name,
                dimension: embed_dimension,
            },
        },
        memory: advanced.memory,
        tools: ToolPermissions {
            web_search: tools.web_search,
            searxng: tools.searxng.clone(),
            code_exec: false,
            read_file: false,
            code_exec_sandbox: Default::default(),
            read_file_dirs: Default::default(),
        },
        web_ui: WebUiConfig::default(),
    };

    Ok(config)
}
