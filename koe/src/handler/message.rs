use crate::context_store;
use crate::regex::url_regex;
use crate::status::VoiceConnectionStatusMap;
use crate::voice_client::VoiceClient;
use aho_corasick::{AhoCorasickBuilder, MatchKind};
use anyhow::Result;
use chrono::Duration;
use discord_md::generate::MarkdownToString;
use koe_db::dict::GetAllOption;
use koe_db::redis;
use log::trace;
use serenity::{
    client::Context,
    model::{channel::Message, id::GuildId},
};

pub async fn handle_message(ctx: &Context, msg: Message) -> Result<()> {
    let guild_id = match msg.guild_id {
        Some(id) => id,
        None => return Ok(()),
    };

    let voice_client = context_store::extract::<VoiceClient>(ctx).await?;
    if !voice_client.is_connected(ctx, guild_id).await? {
        return Ok(());
    }

    let status_map = context_store::extract::<VoiceConnectionStatusMap>(ctx).await?;
    let mut status = match status_map.get_mut(&guild_id) {
        Some(status) => status,
        None => return Ok(()),
    };

    // Skip message from Koe itself
    if msg.author.id == ctx.cache.current_user_id().await {
        return Ok(());
    }

    // Skip message that starts with semicolon
    if msg.content.starts_with(';') {
        return Ok(());
    }

    if status.bound_text_channel == msg.channel_id {
        let text = build_read_text(ctx, guild_id, &msg, &status.last_message_read).await?;

        trace!("Queue reading {:?}", &text);
        status.speech_queue.push(text)?;

        status.last_message_read = Some(msg);
    }

    Ok(())
}

async fn build_read_text(
    ctx: &Context,
    guild_id: GuildId,
    msg: &Message,
    last_msg: &Option<Message>,
) -> Result<String> {
    let mut text = String::new();

    if should_read_author_name(msg, last_msg) {
        let author_name = build_author_name(ctx, msg).await;
        text.push_str(&remove_url(&author_name));
        text.push('。');
    }

    let message_with_mentions_replaced = msg.content_safe(&ctx.cache).await;
    let plain_text_message = discord_md::parse(&message_with_mentions_replaced).to_plain_string();
    text.push_str(&remove_url(&plain_text_message));

    let text = replace_words_on_dict(ctx, guild_id, &text).await?;

    // 文字数を60文字に制限
    if text.chars().count() > 60 {
        Ok(text.chars().take(60 - 5).collect::<String>() + "、以下 略")
    } else {
        Ok(text)
    }
}

fn should_read_author_name(msg: &Message, last_msg: &Option<Message>) -> bool {
    let last_msg = match last_msg {
        Some(msg) => msg,
        None => return true,
    };

    msg.author != last_msg.author || (msg.timestamp - last_msg.timestamp) > Duration::seconds(10)
}

async fn build_author_name(ctx: &Context, msg: &Message) -> String {
    msg.author_nick(&ctx.http)
        .await
        .unwrap_or_else(|| msg.author.name.clone())
}

async fn replace_words_on_dict(ctx: &Context, guild_id: GuildId, text: &str) -> Result<String> {
    let client = context_store::extract::<redis::Client>(ctx).await?;
    let mut conn = client.get_async_connection().await?;

    let dict = koe_db::dict::get_all(
        &mut conn,
        GetAllOption {
            guild_id: guild_id.to_string(),
        },
    )
    .await?;

    let dict_list = dict.into_iter().collect::<Vec<_>>();
    let word_list = dict_list.iter().map(|(word, _)| word).collect::<Vec<_>>();
    let read_as_list = dict_list
        .iter()
        .map(|(_, read_as)| read_as)
        .collect::<Vec<_>>();

    let ac = AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .build(word_list);

    Ok(ac.replace_all(text, &read_as_list))
}

/// メッセージのURLを除去
fn remove_url(text: &str) -> String {
    url_regex().replace_all(text, "、").into()
}
