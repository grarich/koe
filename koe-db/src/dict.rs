use anyhow::{bail, Result};
use redis::aio::Connection;
use redis::AsyncCommands;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct InsertOption {
    pub guild_id: String,
    pub word: String,
    pub read_as: String,
}

#[derive(Debug, Clone)]
pub enum InsertResponse {
    Success,
    WordAlreadyExists,
}

/// 辞書に語句を追加する
pub async fn insert(connection: &mut Connection, option: InsertOption) -> Result<InsertResponse> {
    let resp = connection
        .hset_nx(dict_key(&option.guild_id), option.word, option.read_as)
        .await?;

    Ok(match resp {
        0 => InsertResponse::WordAlreadyExists,
        1 => InsertResponse::Success,
        x => bail!("Unknown HSETNX response from Redis: {}", x),
    })
}

#[derive(Debug, Clone)]
pub struct RemoveOption {
    pub guild_id: String,
    pub word: String,
}

#[derive(Debug, Clone)]
pub enum RemoveResponse {
    Success,
    WordDoesNotExist,
}

/// 辞書から語句を削除する
pub async fn remove(connection: &mut Connection, option: RemoveOption) -> Result<RemoveResponse> {
    let resp = connection
        .hdel(dict_key(&option.guild_id), option.word)
        .await?;

    Ok(match resp {
        0 => RemoveResponse::WordDoesNotExist,
        1 => RemoveResponse::Success,
        x => bail!("Unknown HDEL response from Redis: {}", x),
    })
}

#[derive(Debug, Clone)]
pub struct GetAllOption {
    pub guild_id: String,
}

/// 辞書を[`HashMap`]として返す
/// 辞書が存在しないときは空の[`HashMap`]を返す
pub async fn get_all(
    connection: &mut Connection,
    option: GetAllOption,
) -> Result<HashMap<String, String>> {
    let resp = connection.hgetall(dict_key(&option.guild_id)).await?;
    Ok(resp)
}

fn dict_key(guild_id: &str) -> String {
    format!("guild:{}:dict", guild_id)
}
