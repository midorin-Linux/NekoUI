use std::collections::VecDeque;

use nekoai_memory::store::RecalledMemory;
use tracing::debug;

use crate::session::{ConversationTurn, Session};

pub struct Context {
    pub system_prompt: String,
    pub turns: VecDeque<ConversationTurn>,
    pub user_message: String,
}

pub struct ContextManager {
    base_system_prompt: String,
    max_tokens: usize,
    #[allow(dead_code)]
    compaction_threshold: f32,
}

impl ContextManager {
    pub fn new(base_system_prompt: String, max_tokens: usize, compaction_threshold: f32) -> Self {
        Self {
            base_system_prompt,
            max_tokens,
            compaction_threshold,
        }
    }

    pub async fn build(
        &self,
        session: &Session,
        input: &str,
        recalled_memory: &RecalledMemory,
        caller_user_id: Option<String>,
        caller_guild_id: Option<u64>,
    ) -> Context {
        debug!(
            input_len = input.len(),
            session_turns = session.turns.len(),
            max_tokens = self.max_tokens,
            "building prompt context"
        );
        let mut turns = session.turns.clone();

        let max_turns = (self.max_tokens / 512).max(1);
        if turns.len() > max_turns {
            let drain_count = turns.len() - max_turns;
            for _ in 0 .. drain_count {
                turns.pop_front();
            }
            debug!(
                drained_turns = drain_count,
                "compacted conversation turns for context"
            );
        }

        let channel_id = session.key.channel_id.get().to_string();
        let system_prompt = self.build_system_prompt_with_memory(
            recalled_memory,
            caller_user_id,
            caller_guild_id,
            &channel_id,
        );

        Context {
            system_prompt,
            turns,
            user_message: input.to_string(),
        }
    }

    fn build_system_prompt_with_memory(
        &self,
        recalled: &RecalledMemory,
        caller_user_id: Option<String>,
        caller_guild_id: Option<u64>,
        channel_id: &str,
    ) -> String {
        let user_id = caller_user_id.unwrap_or_else(|| "unknown".to_string());
        let guild_id = caller_guild_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let channel_id = channel_id.to_string();

        let base_system_prompt = self
            .base_system_prompt
            .replace("{guild_name}", "unknown")
            .replace("{channel_name}", "unknown")
            .replace("{category}", "unknown")
            .replace("{user_name}", "unknown")
            .replace("{user_id}", &user_id)
            .replace("{guild_id}", &guild_id)
            .replace("{channel_id}", &channel_id)
            .replace("{roles}", "unknown");

        let mut prompt = String::from("<nekoai_prompt>\n");
        prompt.push_str("  <system_instruction>");
        prompt.push_str(&escape_xml(&base_system_prompt));
        prompt.push_str("</system_instruction>\n");
        prompt.push_str("  <caller_context>\n");
        prompt.push_str(&format!("    <guild_id>{}</guild_id>\n", guild_id));
        prompt.push_str(&format!("    <channel_id>{}</channel_id>\n", channel_id));
        prompt.push_str(&format!("    <user_id>{}</user_id>\n", user_id));
        prompt.push_str("  </caller_context>\n");

        if !recalled.long_term.is_empty() {
            prompt.push_str("  <important_memories>\n");
            for mem in &recalled.long_term {
                prompt.push_str("    <memory>");
                prompt.push_str(&escape_xml(&mem.content));
                prompt.push_str("</memory>\n");
            }
            prompt.push_str("  </important_memories>\n");
        }

        if !recalled.mid_term.is_empty() {
            prompt.push_str("  <past_conversations>\n");
            for summary in &recalled.mid_term {
                prompt.push_str("    <conversation>");
                prompt.push_str(&escape_xml(&summary.content));
                prompt.push_str("</conversation>\n");
            }
            prompt.push_str("  </past_conversations>\n");
        }

        prompt.push_str("</nekoai_prompt>");
        prompt
    }
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
