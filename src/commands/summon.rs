use crate::{
    handlers::{IdleHandler, TrackEndHandler},
    strings::{FAIL_AUTHOR_NOT_FOUND, FAIL_HERE, JOINING},
    utils::create_response,
};
use serenity::{
    client::Context,
    model::interactions::application_command::ApplicationCommandInteraction,
    prelude::{Mentionable, SerenityError},
};
use songbird::{Event, TrackEvent};
use std::time::Duration;

pub async fn summon(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
    send_reply: bool,
) -> Result<(), SerenityError> {
    let guild_id = interaction.guild_id.unwrap();
    let guild = ctx.cache.guild(guild_id).await.unwrap();

    let manager = songbird::get(ctx).await.unwrap();

    let channel_opt = guild
        .voice_states
        .get(&interaction.user.id)
        .and_then(|voice_state| voice_state.channel_id);

    let channel_id = match channel_opt {
        Some(channel_id) => channel_id,
        None if send_reply => {
            return create_response(&ctx.http, interaction, FAIL_AUTHOR_NOT_FOUND).await
        }
        None => return Ok(()),
    };

    if let Some(call) = manager.get(guild.id) {
        let handler = call.lock().await;
        let has_current_connection = handler.current_connection().is_some();
        drop(handler);

        // bot is already in the channel
        if has_current_connection {
            if send_reply {
                return create_response(&ctx.http, interaction, FAIL_HERE).await;
            }
            return Ok(());
        }

        // bot might have been disconnected manually
        manager.remove(guild.id).await.unwrap();
    }

    // join the channel
    manager.join(guild.id, channel_id).await.1.unwrap();

    // unregister existing events and register idle notifier
    if let Some(call) = manager.get(guild.id) {
        let mut handler = call.lock().await;

        handler.remove_all_global_events();

        handler.add_global_event(
            Event::Periodic(Duration::from_secs(1), None),
            IdleHandler {
                http: ctx.http.clone(),
                manager,
                interaction: interaction.clone(),
                limit: 60 * 10,
                count: Default::default(),
            },
        );

        handler.add_global_event(
            Event::Track(TrackEvent::End),
            TrackEndHandler {
                guild_id: guild.id,
                call: call.clone(),
                ctx_data: ctx.data.clone(),
            },
        );
    }

    if send_reply {
        let content = format!("{} **{}**!", JOINING, channel_id.mention());
        return create_response(&ctx.http, interaction, &content).await;
    }

    Ok(())
}
