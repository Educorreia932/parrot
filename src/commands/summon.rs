use crate::{
    connection::get_voice_channel_for_user,
    errors::ParrotError,
    guild::settings::{GuildSettings, GuildSettingsMap},
    handlers::{IdleHandler, TrackEndHandler},
    messaging::message::ParrotMessage,
    utils::create_response,
};
use serenity::{
    client::Context,
    model::{
        application::interaction::application_command::ApplicationCommandInteraction, id::ChannelId,
    },
    prelude::Mentionable,
};
use songbird::{Event, TrackEvent};
use std::{path::Path, time::Duration};

pub async fn summon(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
    send_reply: bool,
    load_settings: bool,
) -> Result<(), ParrotError> {
    let guild_id = interaction.guild_id.unwrap();
    let guild = ctx.cache.guild(guild_id).unwrap();

    let manager = songbird::get(ctx).await.unwrap();
    let channel_opt = get_voice_channel_for_user(&guild, &interaction.user.id);
    let channel_id = channel_opt.unwrap();

    if let Some(call) = manager.get(guild.id) {
        let handler = call.lock().await;
        let has_current_connection = handler.current_connection().is_some();

        if has_current_connection && send_reply {
            // bot is in another channel
            let bot_channel_id: ChannelId = handler.current_channel().unwrap().0.into();
            return Err(ParrotError::AlreadyConnected(bot_channel_id.mention()));
        }
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

    // load existing guild settings to memory
    if load_settings && Path::new("test.json").exists() {
        let mut data = ctx.data.write().await;
        let settings = data.get_mut::<GuildSettingsMap>().unwrap();
        let guild_settings = settings
            .entry(guild_id)
            .or_insert(GuildSettings::new(guild_id));
        guild_settings.load()?;
    }

    if send_reply {
        return create_response(
            &ctx.http,
            interaction,
            ParrotMessage::Summon {
                mention: channel_id.mention(),
            },
        )
        .await;
    }

    Ok(())
}
