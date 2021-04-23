use dotenv::dotenv;
use indexmap::IndexMap;
use rand::rngs::StdRng;
use rand::seq::IteratorRandom;
use rand::FromEntropy;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::string::ToString;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use telegram_bot::{Api, CanSendMessage, Error, Message, MessageKind, UpdateKind};

type UserId = String;
type Emoji = String;
type Quantity = usize;

lazy_static::lazy_static! {
    static ref USERS_EMOJIS: Arc<Mutex<HashMap<UserId, IndexMap<Emoji, Quantity>>>> = Arc::new(Mutex::new(HashMap::new()));

    static ref EMOJI_FILE: String = fs::read_to_string("emojis.csv").unwrap();
    static ref EMOJIS: Vec<&'static str> = EMOJI_FILE.trim().split('\n').collect();
}

enum Command {
    Roll,
    Emojis,
}

impl Command {
    fn from_message(message: &str) -> Self {
        match message {
            "/roll" => Self::Roll,
            "/emojis" => Self::Emojis,
            _ => panic!("message could not be parsed to command"),
        }
    }

    async fn execute(self, api: &Api, message: &Message) -> Result<(), Error> {
        match self {
            Command::Roll => self.roll(api, message).await,
            Command::Emojis => self.emojis(api, message).await,
        }?;

        Ok(())
    }

    async fn roll(&self, api: &Api, message: &Message) -> Result<(), Error> {
        let rolled_emojis = generate_random_emojis();

        let user_id = message.from.id.to_string();

        add_emojis_to_album(user_id, &rolled_emojis);

        api.send(
            message.chat.text(format!(
                "You have rolled: {}",
                rolled_emojis
                    .into_iter()
                    .rev()
                    .collect::<Vec<String>>()
                    .join("")
            )),
        )
        .await?;

        Ok(())
    }

    async fn emojis(&self, api: &Api, message: &Message) -> Result<(), Error> {
        let lock = USERS_EMOJIS.lock().unwrap();

        match lock.get(&message.from.id.to_string()) {
            Some(emojis_map) => {
                let emoji_album = render_emoji_album(emojis_map);

                api.send(
                    message
                        .chat
                        .text(format!("Your emojis:\n\n{}", emoji_album)),
                )
                .await?;
            }
            None => {
                api.send(
                    message
                        .chat
                        .text("You still have no emojis! Type /roll to get some!"),
                )
                .await?;
            }
        };

        Ok(())
    }
}

fn generate_random_emojis() -> Vec<Emoji> {
    let mut rng = StdRng::from_entropy();

    let random_emojis: Vec<String> = EMOJIS
        .iter()
        .choose_multiple(&mut rng, 5)
        .into_iter()
        .map(ToString::to_string)
        .collect();

    random_emojis
}

fn add_emojis_to_album(album: UserId, emojis: &Vec<Emoji>) {
    let mut lock = USERS_EMOJIS.lock().unwrap();
    let user_emojis = lock.entry(album).or_insert(IndexMap::new());

    for emoji in emojis {
        let quantity = user_emojis.entry(emoji.to_owned()).or_insert(0);
        (*quantity) += 1;
    }
}

fn render_emoji_album(emojis_map: &IndexMap<Emoji, Quantity>) -> String {
    emojis_map
        .iter()
        .rev()
        .map(|(emoji, quantity)| {
            std::iter::repeat(emoji.to_owned())
                .take(*quantity)
                .collect::<String>()
        })
        .map(|mut same_emoji_line| {
            same_emoji_line.push_str("   ");
            same_emoji_line
        })
        .collect()
}

async fn handle_message(api: &Api, message: &Message) -> Result<(), Error> {
    if let MessageKind::Text { ref data, .. } = message.kind {
        println!("<{}>: {}", &message.from.id, data);

        Command::from_message(&data[..])
            .execute(&api, message)
            .await?;
    };

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();

    let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");

    let api = Api::new(token);

    let mut stream = api.stream();

    while let Some(update) = stream.next().await {
        let update = update?;

        if let UpdateKind::Message(message) = update.kind {
            handle_message(&api, &message).await?;
        }
    }

    Ok(())
}
