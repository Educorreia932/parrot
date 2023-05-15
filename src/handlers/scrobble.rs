use std::env;
use std::sync::Arc;

use rustfm_scrobble::{Scrobble, Scrobbler};
use serenity::{
    async_trait,
    cache::Cache,
    model::id::GuildId,
    prelude::{Mutex, RwLock, TypeMap},
};
use songbird::{Call, Event, EventContext, EventHandler};

use crate::connection::get_voice_channel_for_user;
use crate::errors::ParrotError;
use crate::guild::settings::GuildSettingsMap;

pub struct ScrobbleHandler {
    pub ctx_data: Arc<RwLock<TypeMap>>,
    pub ctx_cache: Arc<Cache>,
    pub guild_id: GuildId,
    pub call: Arc<Mutex<Call>>,
}

#[async_trait]
impl EventHandler for ScrobbleHandler {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let api_key = &env::var("LASTFM_API_KEY")
            .map_err(|_| ParrotError::Other("missing Last.fm API key")).unwrap();

        let shared_secret = &env::var("LASTFM_SHARED_SECRET")
            .map_err(|_| ParrotError::Other("missing Last.fm shared secret")).unwrap();

        let handler = self.call.lock().await;
        let track = &handler
            .queue()
            .current_queue()
            .get(0)
            .unwrap()
            .clone();
        
        let metadata = track.metadata().clone();
        let artist = &metadata.artist.unwrap();
        let title = &metadata.title.unwrap();

        let guild = self.ctx_cache.guild(&self.guild_id).unwrap();
        let voice_states = &guild.voice_states;
        let bot_id = self.ctx_cache.current_user_id();
        let bot_channel = get_voice_channel_for_user(&guild, &bot_id).unwrap();

        // Get users connected to the same voice channel as the bot
        let voice_states_in_channel = voice_states.iter()
            .filter(|&(_, state)| state.channel_id == Some(bot_channel))
            .map(|(_, state)| state.user_id)
            .collect::<Vec<_>>();

        let mut data = self.ctx_data.write().await;
        let settings = data.get_mut::<GuildSettingsMap>().unwrap();
        let guild_settings = settings.get_mut(&guild.id).unwrap();

        // Scrobble track for every user
        for user in voice_states_in_channel {
            // Check if user has registered Last.fm
            if !guild_settings.lastfm_users.contains_key(&user) {
                continue;
            }
            
            let session_key = guild_settings.lastfm_users.get(&user).unwrap();
            let mut scrobbler = Scrobbler::new(&api_key, &shared_secret);

            scrobbler.authenticate_with_session_key(session_key);

            let track = Scrobble::new(&artist, &title, "");

            scrobbler.scrobble(&track).ok()?;

            println!("[INFO] Scrobbled track!");
        }

        None
    }
}
