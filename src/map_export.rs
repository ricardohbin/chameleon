//! Full-world render to an offscreen target; temporary annotations use their own layer.
use bevy::{prelude::*, camera::{RenderTarget, visibility::RenderLayers},
    render::{render_resource::TextureFormat, view::{ColorGrading, screenshot::{Screenshot, ScreenshotCaptured}}},
    sprite::Anchor, tasks::{AsyncComputeTaskPool, Task, futures::check_ready}};
use crate::{terrain::{Terrain, Area}, mobs::{self, Mob}, boss::BossKind,
    terrain_art::{CanvasTask, TileTask}, debug_overlay::DebugInfo};

const LABEL_LAYER: usize = 7;
const EXPORT_WORLD: usize = 8;
const EXPORT_COLOR: usize = 9;
#[derive(Component)]
pub(crate) struct ExportCamera;
#[derive(Component)]
struct ExportPart;
#[derive(Component)]
struct Feedback;
#[derive(Resource, Default)]
struct RendererReady(bool);
#[derive(Resource, Default)]
struct Export { requested: bool, annotated: bool, frame: u8, image: Option<Handle<Image>>, path: String, status: String,
    pixels: Option<image::RgbImage>, output_size: UVec2, tile_size: UVec2, tile_origin: UVec2, top_left: Vec2, scale: f32,
    encoding: Option<Task<Result<(),String>>>, notice_frames: u8, compression_started: Option<std::time::Instant> }

pub fn register(app: &mut App) {
    app.init_resource::<Export>().init_resource::<RendererReady>().add_systems(Startup, feedback)
        .add_systems(Update, export.after(crate::debug_overlay::toggle))
        .add_systems(Update, hide_empty_feedback)
        .add_systems(PostUpdate, export_layers.before(bevy::camera::visibility::VisibilitySystems::CheckVisibility));
    app.sub_app_mut(bevy::render::RenderApp).add_systems(bevy::render::ExtractSchedule,
        |mut world: ResMut<bevy::render::MainWorld>, pipelines: Res<bevy::render::render_resource::PipelineCache>| {
            if let Some(mut ready) = world.get_resource_mut::<RendererReady>() {
                ready.0 = pipelines.waiting_pipelines().next().is_none();
            }
        });
}

// Extra layers affect only export cameras. Keep the normal game layers intact.
// Walking ancestors also excludes particles/parts parented to backdrop or character roots.
fn export_layers(mut commands: Commands, state: Res<Export>,
    rendered: Query<(Entity, Option<&RenderLayers>, Has<Sprite>), (Or<(With<Mesh2d>, With<Sprite>, With<Text2d>)>, Without<ExportPart>)>,
    parents: Query<(Option<&ChildOf>, Has<crate::atmosphere::Parallax>, Has<crate::player::Player>, Has<crate::boss::Boss>, Has<crate::terrain_art::TerrainPart>)>) {
    if state.image.is_none() { return; }
    for (entity, layers, sprite) in &rendered {
        let base = layers.cloned().unwrap_or_default().without(EXPORT_WORLD).without(EXPORT_COLOR);
        let mut at = entity;
        let mut skip = sprite;
        while let Ok((parent, parallax, player, boss, terrain_art)) = parents.get(at) {
            if parallax || (state.annotated && (player || boss || terrain_art)) { skip = true; break; }
            let Some(parent) = parent else { break; }; at = parent.parent();
        }
        let updated = if skip { base } else if base.intersects(&crate::camera::color_layer()) {
            base.with(EXPORT_COLOR)
        } else { base.with(EXPORT_WORLD) };
        if layers != Some(&updated) { commands.entity(entity).insert(updated); }
    }
}

/// No text, no dark box: the label only shows while an export reports something.
fn hide_empty_feedback(mut label: Single<(&Text, &mut Visibility), With<Feedback>>) {
    let (text, vis) = &mut *label;
    **vis = if text.0.is_empty() { Visibility::Hidden } else { Visibility::Inherited };
}

fn feedback(mut commands: Commands) {
    commands.spawn((Feedback, Text::new(""), TextFont { font_size: FontSize::Px(18.0), ..default() },
        TextColor(Color::WHITE), BackgroundColor(Color::srgba(0.02,0.02,0.02,0.92)),
        Node { position_type: PositionType::Absolute, top: px(20), left: px(20), padding: UiRect::all(px(8)), ..default() }));
}

fn export(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, debug: Res<DebugInfo>, terrain: Res<Terrain>, ready: Res<RendererReady>,
    mut state: ResMut<Export>, mut images: ResMut<Assets<Image>>,
    pending: Query<(), Or<(With<CanvasTask>, With<TileTask>, With<crate::terrain_props::PropJob>)>>, parts: Query<Entity, With<ExportPart>>,
    grading: Single<&ColorGrading, With<crate::camera::MainCamera>>,
    mobs: Query<(&Mob, &GlobalTransform)>, mut feedback: Single<&mut Text, With<Feedback>>,
    mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>,
    tiles: Query<(&Sprite,&GlobalTransform),With<crate::terrain_art::Layer>>,
    mut cameras: Query<&mut Transform,With<ExportCamera>>) {
    if keys.just_pressed(KeyCode::F10) && !state.requested && state.image.is_none() && state.frame != 30 && state.encoding.is_none() {
        state.requested = true; state.annotated = debug.on; state.frame = 0;
        state.status = if state.annotated { "Exporting annotated map: preparing basic terrain..." } else {
            "Exporting large map: waiting for terrain tiles...\nNext: render sections, then WebP compression. Compression can take tens of seconds or longer."
        }.into();
        info!("{}", state.status);
    }
    ***feedback = state.status.clone();
    if state.frame == 30 || state.encoding.is_some() {
        if let Some(result)=state.encoding.as_mut().and_then(check_ready) {
            state.encoding=None; state.compression_started=None; state.frame=0;
            state.status=match result {Ok(())=>format!("Map saved: {}",state.path),Err(e)=>format!("Map export failed: {e}")};
            info!("{}",state.status);
        } else {
            let elapsed=state.compression_started.map_or(0,|started|started.elapsed().as_secs());
            state.status=format!("Map rendered. Compressing {} x {} to WebP... ({}s elapsed)\nLarge images can take tens of seconds or longer. You can keep playing.\nWait for the saved-file message before closing the game.",state.output_size.x,state.output_size.y,elapsed);
            // Present the notice for two complete frames before starting compression.
            // The expensive native encoder runs on a worker, never in a screenshot observer.
            if state.encoding.is_none() {
                if state.notice_frames == 0 {
                    for e in &parts {commands.entity(e).despawn();}
                    state.image=None;
                    info!("{}",state.status);
                }
                if state.notice_frames < 2 {state.notice_frames+=1;} else {
                    let pixels=state.pixels.take().expect("rendered map pixels queued for compression");
                    let path=state.path.clone(); let lossless=state.annotated;
                    state.compression_started=Some(std::time::Instant::now());
                    state.encoding=Some(AsyncComputeTaskPool::get().spawn(async move {write_webp(&pixels,&path,lossless)}));
                }
            }
        }
        ***feedback=state.status.clone();
        return;
    }
    if state.image.is_some() {
        if state.frame == 20 {
            let center = state.top_left + Vec2::new(state.tile_origin.x as f32+state.tile_size.x as f32/2.0,
                -(state.tile_origin.y as f32+state.tile_size.y as f32/2.0))*state.scale;
            for mut tf in &mut cameras { tf.translation = center.extend(0.0); }
            state.frame = 0;
            let columns=state.output_size.x.div_ceil(state.tile_size.x);
            let sections=columns*state.output_size.y.div_ceil(state.tile_size.y);
            let section=state.tile_origin.y/state.tile_size.y*columns+state.tile_origin.x/state.tile_size.x+1;
            state.status = format!("Rendering large map: section {section}/{sections}...\nNext: WebP compression, which can take tens of seconds or longer.");
        }
        if state.frame <= 8 && !ready.0 { state.frame = 0; return; }
        if state.frame < 8 { state.frame += 1; }
        else if state.frame == 8 {
            state.frame += 1;
            commands.spawn(Screenshot::image(state.image.clone().unwrap())).observe(
                move |shot: On<ScreenshotCaptured>, mut state: ResMut<Export>| {
                    let result = shot.image.clone().try_into_dynamic().map_err(|e| format!("{e:?}"))
                        .and_then(|im| {
                            if state.annotated {
                                state.pixels=Some(im.to_rgb8());
                                return Ok(true);
                            }
                            let rgb = im.to_rgb8();
                            let origin = state.tile_origin; let full = state.output_size;
                            let target = state.pixels.as_mut().ok_or("Missing WebP buffer")?;
                            stitch_tile(target,&rgb,origin);
                            state.tile_origin.x += state.tile_size.x;
                            if state.tile_origin.x >= full.x { state.tile_origin.x=0; state.tile_origin.y += state.tile_size.y; }
                            if state.tile_origin.y < full.y { return Ok(false); }
                            Ok(true)
                        });
                    if matches!(result,Ok(false)) { state.frame=20; return; }
                    match result {
                        Ok(_)=> {state.notice_frames=0; state.frame=30;}
                        Err(e)=> {state.status=format!("Map export failed: {e}"); state.pixels=None; state.frame=10; error!("{}",state.status);}
                    }
                });
        } else if state.frame == 10 {
            for e in &parts { commands.entity(e).despawn(); }
            state.image = None;
        }
        return;
    }
    if !state.requested || (!state.annotated && !pending.is_empty()) { return; }
    // Give the request a frame to put visible feedback on screen before preparing labels.
    if state.frame == 0 { state.frame = 1; return; }
    state.requested = false;
    if let Err(e) = std::fs::create_dir_all("screenshots") { state.status = format!("Map export failed: {e}"); return; }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
    state.path = format!("screenshots/map-{}-{stamp}.webp",if state.annotated {"annotated"} else {"full"});
    let mut bounds = terrain.bounds;
    for a in &terrain.areas { bounds = bounds.union(a.rect); }
    for a in &terrain.features { bounds = bounds.union(a.rect); }
    // Pixel-sized margins also provide fallback space for crowded annotation layouts.
    let (bounds,scale,size)=export_layout(bounds,state.annotated);
    let tile_size=if state.annotated {size} else {size.min(UVec2::splat(4096))};
    state.output_size=size; state.tile_size=tile_size; state.tile_origin=UVec2::ZERO;
    state.top_left=Vec2::new(bounds.min.x,bounds.max.y); state.scale=scale;
    state.pixels=if state.annotated {None} else {Some(image::RgbImage::new(size.x,size.y))};
    let center=if state.annotated {bounds.center()} else {state.top_left+Vec2::new(tile_size.x as f32/2.0,-(tile_size.y as f32)/2.0)*scale};
    let image = images.add(Image::new_target_texture(tile_size.x,tile_size.y,TextureFormat::Rgba8UnormSrgb,None));
    let projection = Projection::Orthographic(OrthographicProjection { scale, ..OrthographicProjection::default_2d() });
    commands.spawn((ExportPart, ExportCamera, Camera2d, Camera {order: -2, clear_color: ClearColorConfig::Custom(Color::srgb(0.025,0.035,0.05)), ..default()},
        RenderTarget::Image(image.clone().into()), projection.clone(), if state.annotated {ColorGrading::default()} else {grading.clone()},
        Transform::from_translation(center.extend(0.0)), RenderLayers::layer(EXPORT_WORLD), Msaa::Off));
    commands.spawn((ExportPart, ExportCamera, Camera2d, Camera {order: -1, clear_color: ClearColorConfig::None, ..default()},
        RenderTarget::Image(image.clone().into()), projection,
        RenderLayers::from_layers(&[EXPORT_COLOR, LABEL_LAYER]),
        Transform::from_translation(center.extend(0.0)), Msaa::Off));
    if state.annotated {
        basic_terrain(&mut commands,&terrain,&mut meshes,&mut materials);
        let live: Vec<_> = mobs.iter().filter(|(m,_)| m.hp > 0).map(|(m,t)| (m.kind,t.translation().truncate())).collect();
        annotations(&mut commands,&terrain,bounds,scale*1.7,&live);
    } else {
        // Reuse the painted textures and live contrast tint as textured quads.
        // Mesh quads share the reliable export path used by the creatures.
        for (sprite,tf) in &tiles {
            let Some(size)=sprite.custom_size else {continue;};
            let material=materials.add(ColorMaterial {color:sprite.color,texture:Some(sprite.image.clone()),..default()});
            commands.spawn((ExportPart,Mesh2d(meshes.add(Rectangle::new(size.x,size.y))),MeshMaterial2d(material),tf.compute_transform(),RenderLayers::layer(EXPORT_WORLD)));
        }
    }
    state.image = Some(image); state.frame = 0;
    state.status = format!("Rendering {} x {} map...\nNext: {} WebP compression. Large images can take tens of seconds or longer.",size.x,size.y,if state.annotated {"lossless"} else {"lossy"});
}

fn write_webp(rgb: &image::RgbImage,path: &str,lossless: bool) -> Result<(),String> {
    if let Some(parent)=std::path::Path::new(path).parent() {std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
    let compressed=webp::Encoder::from_rgb(rgb.as_raw(),rgb.width(),rgb.height())
        .encode_simple(lossless,if lossless {75.0} else {90.0}).map_err(|e|format!("{e:?}"))?;
    std::fs::write(path,&*compressed).map_err(|e|e.to_string())
}

fn stitch_tile(target: &mut image::RgbImage,tile: &image::RgbImage,origin: UVec2) {
    let width=tile.width().min(target.width()-origin.x);
    let height=tile.height().min(target.height()-origin.y);
    for y in 0..height {
        let src=(y*tile.width()*3) as usize;
        let dst=(((origin.y+y)*target.width()+origin.x)*3) as usize;
        target.as_mut()[dst..dst+width as usize*3].copy_from_slice(&tile.as_raw()[src..src+width as usize*3]);
    }
}

fn export_layout(bounds: Rect,annotated: bool) -> (Rect,f32,UVec2) {
    let unit=(bounds.size().max_element()/7000.0).max(1.0);
    let annotated_bounds=Rect::from_corners(bounds.min-Vec2::new(600.0,500.0)*unit,bounds.max+Vec2::new(600.0,320.0)*unit);
    let annotated_scale=(annotated_bounds.size().max_element()/8192.0).max(unit);
    let annotated_size=(annotated_bounds.size()/annotated_scale).ceil().as_uvec2().min(UVec2::splat(8192));
    let plain_max=(annotated_size.max_element()*2).min(16383);
    let scale=if annotated {annotated_scale} else {bounds.size().max_element()/plain_max as f32};
    let bounds=if annotated {annotated_bounds} else {bounds};
    let cap=if annotated {8192} else {16383};
    let size=(bounds.size()/scale).ceil().as_uvec2().max(UVec2::ONE).min(UVec2::splat(cap));
    (bounds,scale,size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webp_lossless_preserves_pixels_and_lossy_uses_the_right_codec() {
        let pixels=image::RgbImage::from_fn(31,19,|x,y|image::Rgb([(x*7) as u8,(y*11) as u8,(x*y) as u8]));
        for lossless in [true,false] {
            let path=std::env::temp_dir().join(format!("chameleon-webp-{}-{lossless}.webp",std::process::id()));
            write_webp(&pixels,path.to_str().unwrap(),lossless).unwrap();
            let bytes=std::fs::read(&path).unwrap();
            let features=webp::BitstreamFeatures::new(&bytes).unwrap();
            assert_eq!((features.width(),features.height()),(31,19));
            assert_eq!(matches!(features.format(),Some(webp::BitstreamFormat::Lossless)),lossless);
            if lossless {assert_eq!(&*webp::Decoder::new(&bytes).decode().unwrap(),pixels.as_raw().as_slice());}
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn partial_edge_tile_does_not_overwrite_neighboring_pixels() {
        let mut target=image::RgbImage::new(5,3);
        let tile=image::RgbImage::from_pixel(3,3,image::Rgb([20,40,60]));
        stitch_tile(&mut target,&tile,UVec2::new(4,2));
        for (x,y,pixel) in target.enumerate_pixels() {
            assert_eq!(pixel.0,if (x,y)==(4,2) {[20,40,60]} else {[0,0,0]});
        }
    }

    #[test]
    fn export_sizes_fit_content_and_keep_webp_within_its_limit() {
        let bounds=Rect::new(-20000.0,-10000.0,20000.0,10000.0);
        let (_,_,annotated)=export_layout(bounds,true);
        let (plain_bounds,scale,plain)=export_layout(bounds,false);
        assert_eq!(annotated.max_element(),8192);
        assert_eq!(plain.max_element(),16383);
        assert_eq!(plain_bounds,bounds);
        assert!((plain.as_vec2()*scale-bounds.size()).abs().max_element()<=scale);
    }
}

pub(crate) fn save_lossless_webp(path: impl Into<String>) -> impl FnMut(On<ScreenshotCaptured>) {
    let path=path.into();
    move |shot| {
        let result=shot.image.clone().try_into_dynamic().map_err(|e|format!("{e:?}"))
            .and_then(|image|write_webp(&image.to_rgb8(),&path,true));
        match result {Ok(())=>info!("Screenshot saved: {path}"),Err(e)=>error!("Screenshot failed: {path}: {e}")}
    }
}

fn basic_terrain(commands: &mut Commands,t: &Terrain,meshes: &mut Assets<Mesh>,materials: &mut Assets<ColorMaterial>) {
    use crate::{sketch::Cell, terrain::PX};
    for (cell,color) in [(Cell::Solid,Color::srgb(0.35,0.40,0.45)),(Cell::Back,Color::srgb(0.105,0.135,0.165))] {
        let mut tris = Vec::new();
        for y in 0..t.size().y {
            let mut x=0;
            while x<t.size().x {
                if t.cell(IVec2::new(x,y)) != cell { x+=1; continue; }
                let start=x; while x<t.size().x && t.cell(IVec2::new(x,y))==cell {x+=1;}
                let a=t.cell_min(IVec2::new(start,y));
                let b=a+Vec2::new((x-start) as f32*PX,PX);
                let c=Vec2::new(a.x,b.y);let d=Vec2::new(b.x,a.y);
                tris.extend([[a,d,b],[a,b,c]]);
            }
        }
        commands.spawn((ExportPart,Mesh2d(meshes.add(crate::shapes::triangles(&tris))),
            MeshMaterial2d(materials.add(color)),Transform::from_xyz(0.0,0.0,-1.0),RenderLayers::layer(EXPORT_WORLD)));
    }
}

fn rectangle(commands: &mut Commands,size: Vec2,color: Color,tf: Transform) {
    commands.queue(move |world: &mut World| {
        let mesh=world.resource_mut::<Assets<Mesh>>().add(Rectangle::new(size.x,size.y));
        let mat=world.resource_mut::<Assets<ColorMaterial>>().add(color);
        world.spawn((ExportPart,Mesh2d(mesh),MeshMaterial2d(mat),tf,RenderLayers::layer(LABEL_LAYER)));
    });
}

fn wrap(s: &str) -> String {
    let mut out = String::new(); let mut n = 0;
    for word in s.split_whitespace() { if n + word.len() > 65 { out.push('\n'); n=0; }
        if n>0 {out.push(' ');n+=1;} out.push_str(word);n+=word.len(); }
    out
}

fn panel(commands: &mut Commands, text: String, rect: Rect, scale: f32, color: Color) {
    rectangle(commands,rect.size(),Color::srgb(0.025,0.025,0.03),Transform::from_translation(rect.center().extend(200.0)));
    commands.spawn((ExportPart,Text2d::new(text),TextFont {font_size: FontSize::Px(12.0*scale),..default()},
        TextColor(color),Anchor::TOP_LEFT,
        Transform::from_translation(Vec3::new(rect.min.x+5.0*scale,rect.max.y-4.0*scale,201.0)),RenderLayers::layer(LABEL_LAYER)));
}
fn line(commands: &mut Commands,a: Vec2,b: Vec2,scale: f32,color: Color) {
    rectangle(commands,Vec2::new(a.distance(b),scale),color,
        Transform::from_translation(((a+b)*0.5).extend(199.0)).with_rotation(Quat::from_rotation_z((b-a).to_angle())));
}
fn outline(commands: &mut Commands,r: Rect,s: f32,c: Color) {
    let p=[r.min,Vec2::new(r.max.x,r.min.y),r.max,Vec2::new(r.min.x,r.max.y)];
    for i in 0..4 {line(commands,p[i],p[(i+1)%4],s,c);}
}
fn marker(commands: &mut Commands, point: Vec2, scale: f32, color: Color) {
    rectangle(commands,Vec2::splat(17.0*scale),Color::BLACK,Transform::from_translation(point.extend(210.0)));
    rectangle(commands,Vec2::splat(13.0*scale),color,Transform::from_translation(point.extend(211.0)));
}

fn ecosystem(t: &Terrain,a: &Area) -> Option<BossKind> {
    if a.rect.contains(t.spawn) {return None;}
    let bosses: Vec<_> = [(BossKind::Roach,t.roach),(BossKind::Wasp,t.wasp),(BossKind::Dragonfly,t.dragonfly),(BossKind::Spider,t.spider),(BossKind::Centipede,t.centipede),(BossKind::Frog,t.frog),(BossKind::Grasshopper,t.grasshopper),(BossKind::Rat,t.rat),(BossKind::Mantis,t.mantis)].into_iter().filter_map(|(k,p)|Some((k,p?))).collect();
    bosses.iter().find(|(_,p)|a.rect.contains(*p)).or_else(||bosses.iter().min_by(|a1,b1| {
        let d=|p: Vec2| p.distance(p.clamp(a.rect.min,a.rect.max));d(a1.1).total_cmp(&d(b1.1))
    })).map(|(k,_)|*k)
}

fn annotations(commands: &mut Commands,t: &Terrain,bounds: Rect,s: f32,live: &[(mobs::MobKind,Vec2)]) {
    let white=Color::srgb(0.95,0.95,0.9); let mut occupied=Vec::new();
    for (i,(point,spec,kind)) in [(t.roach,&crate::roach::SPEC,BossKind::Roach),(t.wasp,&crate::wasp::SPEC,BossKind::Wasp),(t.dragonfly,&crate::dragonfly::SPEC,BossKind::Dragonfly),(t.spider,&crate::spider::SPEC,BossKind::Spider),(t.centipede,&crate::centipede::SPEC,BossKind::Centipede),(t.frog,&crate::frog::SPEC,BossKind::Frog),(t.grasshopper,&crate::grasshopper::SPEC,BossKind::Grasshopper),(t.rat,&crate::king_rat::SPEC,BossKind::Rat),(t.mantis,&crate::mantis::SPEC,BossKind::Mantis)].into_iter().enumerate() {
        let Some(point)=point else {continue};
        let rect=Rect::from_center_size(Vec2::new(bounds.min.x+(240.0+i as f32*470.0)*s,bounds.max.y-90.0*s),Vec2::new(450.0,145.0)*s);
        panel(commands,format!("{}\nSpawn: {:.0}, {:.0} (colored square)\nHP: {} | radius: {:.1} | aggro: {:.0}\nFood: {:.1} | reward: {}\nEnrages at half HP",spec.name,point.x,point.y,spec.hp,spec.radius,spec.aggro,spec.food,kind.power()),rect,s,spec.color);
        occupied.push(rect);
        marker(commands,point,s,spec.color);
    }
    let red = Color::srgb(1.0,0.08,0.06);
    let spawn = crate::player::spawn_point(t);
    marker(commands,spawn,s,red);
    let player_rect = Rect::from_center_size(Vec2::new(bounds.min.x+1180.0*s,bounds.max.y-90.0*s),Vec2::new(450.0,145.0)*s);
    panel(commands,format!("PLAYER - INITIAL STATE (red square)\nSpawn: {:.0}, {:.0}\nHealth: {hp}/{hp}; shadow health: 0\nGround movement, jump and claw slash\nDash/spit locked; no colors earned\nFacing right; alive; flight off",spawn.x,spawn.y,hp=crate::player::MAX_HP),player_rect,s,red);
    occupied.push(player_rect);
    let mut labels: Vec<(Rect,String,Color)> = Vec::new();
    for a in &t.areas {
        let here: Vec<_>=live.iter().filter(|(_,p)|a.rect.contains(*p)).collect();
        let mut text=format!("{}\nTileset: {:?}; tag: {}\nLive: {} / {} ({:.1}% of all creatures)\nSlide only: {}; fall absorption: {:.0}%",wrap(&a.name),a.style,crate::terrain::style_tag(&a.name),here.len(),live.len(),100.0*here.len() as f32/live.len().max(1) as f32,a.slide,a.style.softness()*100.0);
        if let Some(owner)=ecosystem(t,a) {
            text.push_str(&format!("\nEcosystem: {owner:?} | spawn slot shares:"));
            for (kind,share) in mobs::menu(owner,a.style) {
                let n=here.iter().filter(|(k,_)|*k==kind).count();
                text.push_str(&format!("\n{kind:?}: {:.0}% spawn; {n} live ({:.0}%)",share*100.0,n as f32*100.0/here.len().max(1) as f32));
            }
            text.push_str("\nShares are slots; flies swarm, rats pack.\nPlacement depends on space; arenas kept clear.");
        } else {text.push_str("\nNo initial spawns (start region / no boss).");}
        labels.push((a.rect,text,white));
    }
    for f in &t.features {labels.push((f.rect,format!("{}\nSlide only: {}; crumbles: {}",wrap(&f.name),f.slide,f.crumbles),Color::srgb(0.5,0.85,1.0)));}
    for (region,text,color) in labels {
        outline(commands,region,s,color);
        let size=Vec2::new(450.0,text.lines().count() as f32*15.0+12.0)*s;
        let mut best=None;
        // Search inside and near the box, favor homogeneous terrain and avoiding other notes.
        for y in 0..=((bounds.height()-size.y)/(35.0*s)).max(0.0) as usize {
            for x in 0..=((bounds.width()-size.x)/(70.0*s)).max(0.0) as usize {
                let min=bounds.min+Vec2::new(x as f32*70.0,y as f32*35.0)*s;
                let r=Rect::from_corners(min,min+size);
                if occupied.iter().any(|o: &Rect|o.intersect(r).size().min_element()>0.0) {continue;}
                let mut solid=0;
                for iy in 0..5 {for ix in 0..9 {solid+=t.solid_at(r.min+r.size()*Vec2::new(ix as f32/8.0,iy as f32/4.0)) as i32;}}
                let mixed=solid.min(45-solid) as f32;
                let distance=r.center().distance(r.center().clamp(region.min,region.max))/s;
                let score=distance+mixed*160.0+r.center().distance(region.center())/s*0.05;
                if best.is_none_or(|(_,v)|score<v) {best=Some((r,score));}
            }
        }
        let r=best.map(|(r,_)|r).unwrap_or_else(||Rect::from_center_size(region.center(),size));
        let a=r.center().clamp(region.min,region.max);
        let b=a.clamp(r.min,r.max);
        line(commands,a,b,s,color);panel(commands,text,r,s,color);occupied.push(r);
    }
}
