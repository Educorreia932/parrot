use std::env;

use serde::Deserialize;
use serenity::{
    builder::{CreateComponents, CreateInputText},
    client::Context,
    collector::ModalInteractionCollectorBuilder,
    model::{
        application::{
            component::ActionRowComponent,
            interaction::application_command::ApplicationCommandInteraction,
            interaction::InteractionResponseType,
        },
        prelude::component::InputTextStyle,
    },
};
use serenity::futures::StreamExt;
use url_builder::URLBuilder;

use crate::errors::ParrotError;
use crate::guild::settings::GuildSettingsMap;

#[derive(Deserialize)]
struct Session {
    _name: String,
    key: String,
    _subscriber: u32,
}

#[derive(Deserialize)]
struct SessionData {
    session: Session,
}

pub async fn register(
    ctx: &Context,
    interaction: &mut ApplicationCommandInteraction,
) -> Result<(), ParrotError> {
    let api_key = &env::var("LASTFM_API_KEY")
        .map_err(|_| ParrotError::Other("missing Last.fm API key"))?;

    let shared_secret = &env::var("LASTFM_SHARED_SECRET")
        .map_err(|_| ParrotError::Other("missing Last.fm shared secret"))?;

    let url = format!("https://www.last.fm/api/auth/?api_key={api_key}");

    println!("[DEBUG] {url}");

    let mut token_input = CreateInputText::default();

    token_input
        .label("Last.fm token")
        .custom_id("token_input")
        .style(InputTextStyle::Paragraph)
        .placeholder("Paste here the token provided by Last.fm")
        .min_length(32)
        .max_length(32)
        .required(true);

    let mut components = CreateComponents::default();

    components
        .create_action_row(|r| r.add_input_text(token_input));

    interaction
        .create_interaction_response(&ctx.http, |r| {
            r.kind(InteractionResponseType::Modal);
            r.interaction_response_data(|d| {
                d.title("Register Last.fm account");
                d.custom_id("register_lastfm");
                d.set_components(components)
            })
        })
        .await?;

    // collect the submitted data
    let collector = ModalInteractionCollectorBuilder::new(ctx)
        .filter(|int| int.data.custom_id == "register_lastfm")
        .build();

    let guild_id = interaction.guild_id.unwrap();
    let user_id = interaction.user.id;

    collector
        .then(|int| async move {
            let inputs: Vec<_> = int
                .data
                .components
                .iter()
                .flat_map(|r| r.components.iter())
                .collect();

            let session_key = match inputs.get(0) {
                Some(ActionRowComponent::InputText(token_input)) => {
                    let token = &token_input.value;
                    let api_sig = md5::compute(format!("api_key{api_key}methodauth.getSessiontoken{token}{shared_secret}"));

                    let mut url_builder = URLBuilder::new();

                    url_builder.set_protocol("https")
                        .set_host("ws.audioscrobbler.com/2.0")
                        .add_param("method", "auth.getSession")
                        .add_param("token", token)
                        .add_param("api_key", &api_key)
                        .add_param("api_sig", &format!("{:x}", api_sig))
                        .add_param("format", "json");

                    let url = url_builder.build();
                    let response = reqwest::get(url).await.unwrap();
                    let json = response.json::<SessionData>().await.unwrap();

                    json.session.key
                }
                _ => {
                    return;
                }
            };

            let mut data = ctx.data.write().await;
            let settings = data.get_mut::<GuildSettingsMap>().unwrap();
            let guild_settings = settings.get_mut(&guild_id).unwrap();

            guild_settings.add_lastfm_user(user_id, &session_key);
            guild_settings.save().unwrap();

            // it's now safe to close the modal, so send a response to it
            int.create_interaction_response(&ctx.http, |r| {
                r.kind(InteractionResponseType::DeferredUpdateMessage)
            })
                .await
                .ok();
        })
        .collect::<Vec<_>>()
        .await;

    Ok(())
}
