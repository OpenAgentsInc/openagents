use std::path::PathBuf;
use secp256k1::{SecretKey,Secp256k1};
use verse_world::{play::Game,service::{Chamber,auth::Gateway,reach::{self,GrantCheck,GrantRefusal,Server,Carrier}}};
struct Grants(String);
impl GrantCheck for Grants {
fn check(&self,device:&str,grant:&str,epoch:u64,_:u64)->Result<(),GrantRefusal>{if device==self.0 && grant=="01".repeat(32) && epoch==1 {Ok(())}else{Err(GrantRefusal::Unknown)}}
}
#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error>> {
let dir=PathBuf::from(std::env::args().nth(1).unwrap()); std::fs::create_dir_all(dir.join("assets"))?;
let pack=verse_content::compiler::original::generate(&dir.join("assets"))?;
let scene:verse_engine::director::Scene=serde_json::from_slice(&std::fs::read("/home/christopherdavid/work/openagents-verse-audit/assets/verse/original/ritual.json")?)?;
let content=verse_content::remote_content::identity(&pack,&scene,&dir.join("assets"))?;
std::fs::write(dir.join("pack.json"),serde_json::to_vec(&pack)?)?;std::fs::write(dir.join("scene.json"),serde_json::to_vec(&scene)?)?;
let key=SecretKey::from_byte_array([41;32])?;let host=SecretKey::from_byte_array([40;32])?;let secp=Secp256k1::new();
let device=key.x_only_public_key(&secp).0; let mut game=Game::combat_in(scene,false,120)?;game.time=game.scene.cut_at;game.tick(0.,[0.;2])?;
let mut gateway=Gateway::new(Chamber::new(game)?)?.with_content(content)?;gateway.enroll_primary(device.serialize())?;
let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await?;let address=listener.local_addr()?;
let hex=content.iter().map(|b|format!("{b:02x}")).collect::<String>();
let config=serde_json::json!({"websocket":format!("ws://{address}/chamber"),"host":host.x_only_public_key(&secp).0.to_string(),"grant":"01".repeat(32),"epoch":1,"generation":7,"instance":120,"content":hex,"pack":"pack.json","scene":"scene.json","assets":"assets"});
std::fs::write(dir.join("chamber.json"),serde_json::to_vec_pretty(&config)?)?;
println!("Ready {address}");
while !dir.join("start").exists(){tokio::time::sleep(std::time::Duration::from_millis(100)).await;}
let observation=dir.join("pose-state.json");
let tick=Box::new(move |gateway:&mut Gateway,_:f32| {let game=gateway.game();let admission=game.player_admission(14).unwrap();let value=serde_json::json!({"position":game.actor_position(14).unwrap().to_array(),"epoch":admission.epoch(),"sequence":admission.accepted_sequence(),"life":admission.actor(),"tick":game.authority_tick});std::fs::write(&observation,serde_json::to_vec(&value).unwrap()).unwrap();});
let stop=dir.join("stop");let exit=reach::serve_ticked(listener,Server::new(host,7,Grants(device.to_string()),Carrier::WebSocket),gateway,None,tick,async move{loop{if stop.exists(){break}tokio::time::sleep(std::time::Duration::from_millis(100)).await;}}).await;
std::fs::write(dir.join("host-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"failure":exit.failure,"stats":format!("{:?}",exit.stats)}))?)?;
Ok(())
}
