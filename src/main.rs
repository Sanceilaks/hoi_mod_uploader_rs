use jomini::JominiDeserialize;
use std::{path::PathBuf, sync::mpsc::TryRecvError};

use clap::arg;

#[derive(JominiDeserialize)]
struct Descriptor {
    tags: Option<Vec<String>>,
    name: String,
    remote_file_id: Option<String>,
}

impl Descriptor {
    fn from_file(path: &PathBuf) -> Self {
        let content = std::fs::read_to_string(path).unwrap();
        jomini::text::de::from_utf8_slice(content.as_bytes()).unwrap()
    }
}

struct UploadConfig {
    visible: bool,
    thumbnail: Option<PathBuf>,
    description: String,
    path: PathBuf,
    changenotes: Option<String>,
}

fn publish_content(
    client: &steamworks::Client,
    descriptor: &Descriptor,
    file_id: u64,
    cfg: UploadConfig,
) {
    println!("Uploading content...");

    let mut _upload_handle = client
        .ugc()
        .start_item_update(394360.into(), file_id.into())
        .content_path(&cfg.path)
        .title(&descriptor.name.clone())
        .description(&cfg.description)
        .visibility(if cfg.visible {
            steamworks::PublishedFileVisibility::Public
        } else {
            steamworks::PublishedFileVisibility::Private
        });

    if let Some(thumbnail) = cfg.thumbnail {
        _upload_handle = _upload_handle.preview_path(&thumbnail);
    }

    if let Some(tags) = descriptor.tags.clone() {
        _upload_handle = _upload_handle.tags(tags, false);
    }

    _upload_handle.submit(
        cfg.changenotes.as_deref(),
        |upload_result| match upload_result {
            Ok((published_id, needs_to_agree_to_terms)) => {
                if needs_to_agree_to_terms {
                    println!(
                        "You need to agree to the terms of use before you can upload any files"
                    );
                } else {
                    println!("Uploaded item with id {:?}", published_id);

                    std::process::exit(0);
                }
            }
            Err(e) => {
                println!("Error uploading item: {:?}", e);
            }
        },
    );
}

fn main() {
    let cli = clap::Command::new("Steamworks")
        .arg(arg!(--path <PATH> "Path to mod folder").required(true))
        .arg(arg!(--thumbnail <PATH> "Path to thumbnail image").required(false))
        .arg(arg!(--description <DESCRIPTION> "Description of the mod").required(true))
        .arg(arg!(--changenotes <CHANGENOTES> "Changenotes of the mod").required(false))
        .arg(arg!(--visible "Make the mod visible").required(false))
        .arg(arg!(--id <ID> "Steam Workshop ID").required(false))
        .get_matches();

    let (cl, single) = steamworks::Client::init_app(394360).unwrap();

    let callback_thread = std::thread::spawn(move || {
        loop {
            // run callbacks
            single.run_callbacks();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    let path = cli.get_one::<String>("path").unwrap();
    let path = PathBuf::from(path);
    let descriptor = Descriptor::from_file(&path.join("descriptor.mod"));

    let cfg = UploadConfig {
        visible: cli.get_flag("visible"),
        thumbnail: cli.get_one::<PathBuf>("thumbnail").cloned(),
        description: cli.get_one::<String>("description").unwrap().clone(),
        path: path.clone(),
        changenotes: cli.get_one::<String>("changenotes").cloned(),
    };

    let steam_id = cli.get_one::<String>("id").cloned();
    let steam_id = steam_id.map(|s| s.parse::<u64>().unwrap());

    if descriptor.remote_file_id.is_none() && steam_id.is_none() {
        println!("Creating item...");

        cl.ugc().create_item(
            394360.into(),
            steamworks::FileType::Community,
            move |create_result| match create_result {
                Ok((published_id, needs_to_agree_to_terms)) => {
                    if needs_to_agree_to_terms {
                        println!("Please agree to terms first");
                    } else {
                        println!("Published ID: {:?}", published_id);

                        publish_content(&cl, &descriptor, published_id.0, cfg);
                    }
                }
                Err(e) => {
                    println!("Error creating item: {:?}", e);
                }
            },
        )
    } else {
        let file_id = steam_id.unwrap_or_else(|| {
            descriptor
                .remote_file_id
                .as_ref()
                .unwrap()
                .parse::<u64>()
                .unwrap()
        });

        println!("File ID: {:?}", file_id);

        publish_content(&cl, &descriptor, file_id, cfg);
    }

    callback_thread
        .join()
        .expect("Failed to join callback thread");
}
