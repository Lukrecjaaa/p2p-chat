use std::collections::HashSet;

use anyhow::Result;
use chrono::Utc;
use libp2p::PeerId;
use rand::random;
use tracing::debug;
use uuid::Uuid;

use crate::cli::commands::MailboxDeliveryResult;
use crate::types::{DeliveryStatus, Friend, Message};

use super::super::context::CommandContext;
use super::super::resolver::resolve_peer_id;

pub async fn handle_send(parts: &[&str], context: &CommandContext) -> Result<()> {
    if parts.len() < 3 {
        context.emit_chat("Usage: send <peer_id_or_nickname> <message...>");
        return Ok(());
    }

    let destination = parts[1];
    let message_body = parts[2..].join(" ");

    let recipient_peer_id = match resolve_peer_id(destination, context).await {
        Ok(id) => id,
        Err(e) => {
            context.emit_chat(format!("❌ {}", e));
            return Ok(());
        }
    };

    let friend = match context
        .node()
        .friends
        .get_friend(&recipient_peer_id)
        .await?
    {
        Some(f) => f,
        None => {
            context.emit_chat("❌ Friend not found. Add them first with 'friend' command.");
            return Ok(());
        }
    };

    let encrypted_content = match context
        .node()
        .identity
        .encrypt_for(&friend.e2e_public_key, message_body.as_bytes())
    {
        Ok(content) => content,
        Err(e) => {
            context.emit_chat(format!("❌ Encryption failed: {}", e));
            return Ok(());
        }
    };

    let message = Message {
        id: Uuid::new_v4(),
        sender: context.node().identity.peer_id,
        recipient: recipient_peer_id,
        timestamp: Utc::now().timestamp_millis(),
        content: encrypted_content,
        nonce: random(),
        delivery_status: DeliveryStatus::Sending,
    };

    context
        .node()
        .history
        .store_message(message.clone())
        .await?;
    context.node().outbox.add_pending(message.clone()).await?;

    if attempt_direct_delivery(destination, &message, context).await? {
        return Ok(());
    }

    attempt_mailbox_delivery(destination, &message, &friend, context).await
}

async fn attempt_direct_delivery(
    destination: &str,
    message: &Message,
    context: &CommandContext,
) -> Result<bool> {
    match context
        .node()
        .network
        .send_message(message.recipient, message.clone())
        .await
    {
        Ok(()) => {
            context.node().outbox.remove_pending(&message.id).await?;
            context.emit_chat(format!("✅ Message sent directly to {}", destination));
            Ok(true)
        }
        Err(_) => {
            context.emit_chat(format!(
                "⚠️ {} is offline. Attempting mailbox delivery...",
                destination
            ));
            Ok(false)
        }
    }
}

async fn attempt_mailbox_delivery(
    destination: &str,
    message: &Message,
    friend: &Friend,
    context: &CommandContext,
) -> Result<()> {
    let providers = {
        let mut sync_engine = context.node().sync_engine.lock().await;
        let current = sync_engine.get_mailbox_providers().clone();
        if current.is_empty() {
            debug!("No known mailboxes, triggering discovery");
            if let Err(e) = sync_engine.discover_mailboxes().await {
                debug!("Mailbox discovery failed: {}", e);
            }
            sync_engine.get_mailbox_providers().clone()
        } else {
            current
        }
    };

    if !providers.is_empty() {
        return deliver_via_mailboxes(destination, message, friend, context, providers.into_iter())
            .await;
    }

    let emergency_set: HashSet<PeerId> = {
        let sync_engine = context.node().sync_engine.lock().await;
        sync_engine
            .get_emergency_mailboxes()
            .await
            .into_iter()
            .collect()
    };

    if emergency_set.is_empty() {
        context.emit_chat(format!(
            "⚠️ No mailboxes or connected peers available. Message queued for when {} comes online",
            destination
        ));
        return Ok(());
    }

    deliver_via_mailboxes(
        destination,
        message,
        friend,
        context,
        emergency_set.into_iter(),
    )
    .await
}

async fn deliver_via_mailboxes<I>(
    destination: &str,
    message: &Message,
    friend: &Friend,
    context: &CommandContext,
    providers: I,
) -> Result<()>
where
    I: IntoIterator<Item = PeerId>,
{
    let provider_set: HashSet<PeerId> = providers.into_iter().collect();
    match context
        .node()
        .forward_to_mailboxes(message, friend, &provider_set)
        .await
    {
        Ok(MailboxDeliveryResult::Success(count)) => {
            context.node().outbox.remove_pending(&message.id).await?;
            context.emit_chat(format!(
                "📬 Message stored in {} network mailbox(es) for {}",
                count, destination
            ));
        }
        Ok(MailboxDeliveryResult::Failure) => {
            context.emit_chat(format!(
                "⚠️ Mailbox delivery failed. Message queued for retry when {} comes online",
                destination
            ));
        }
        Err(e) => {
            context.emit_chat(format!("⚠️ Mailbox error: {}. Message queued for retry", e));
        }
    }

    Ok(())
}
