use glam::{Quat, Vec3};
use space_soup::renderer::{Color3, Cuboid};
use space_soup_engine::{HandSample, RenderCuboid};

pub fn engine_cuboid_to_render(rc: &RenderCuboid) -> Cuboid {
    let mut c = match rc.style {
        space_soup_engine::CuboidStyle::Solid => Cuboid::solid(
            rc.position,
            rc.half_size,
            Color3(rc.color.0, rc.color.1, rc.color.2, rc.color.3),
        ),
        space_soup_engine::CuboidStyle::Wireframe => Cuboid::wireframe(
            rc.position,
            rc.half_size,
            Color3(
                rc.wire_color.0,
                rc.wire_color.1,
                rc.wire_color.2,
                rc.wire_color.3,
            ),
        ),
        space_soup_engine::CuboidStyle::SolidAndWire => Cuboid::solid_and_wire(
            rc.position,
            rc.half_size,
            Color3(rc.color.0, rc.color.1, rc.color.2, rc.color.3),
            Color3(
                rc.wire_color.0,
                rc.wire_color.1,
                rc.wire_color.2,
                rc.wire_color.3,
            ),
        ),
    };
    c.rotation = rc.rotation;
    c
}

pub fn build_player_overlay(
    world_head_pos: Vec3,
    world_head_rot: Quat,
    left_hand: &HandSample,
    right_hand: &HandSample,
    hand_transform: impl Fn(Vec3, Quat) -> (Vec3, Quat),
) -> Vec<Cuboid> {
    let mut cuboids = Vec::new();

    cuboids.push({
        let mut c = Cuboid::solid_and_wire(
            world_head_pos,
            Vec3::new(0.08, 0.08, 0.10),
            Color3(220, 220, 80, 255),
            Color3(255, 255, 255, 255),
        );
        c.rotation = world_head_rot;
        c
    });

    cuboids.push({
        let forward = world_head_rot * Vec3::new(0.0, 0.0, -1.0);
        let mut c = Cuboid::solid(
            world_head_pos + forward * 0.12,
            Vec3::splat(0.025),
            Color3(255, 80, 80, 255),
        );
        c.rotation = world_head_rot;
        c
    });

    push_hand(
        &mut cuboids,
        left_hand,
        Color3(180, 200, 255, 255),
        &hand_transform,
    );
    push_hand(
        &mut cuboids,
        right_hand,
        Color3(255, 200, 180, 255),
        &hand_transform,
    );

    cuboids
}

fn push_hand(
    cuboids: &mut Vec<Cuboid>,
    hand: &HandSample,
    color: Color3,
    transform: &impl Fn(Vec3, Quat) -> (Vec3, Quat),
) {
    if hand.tracking_active && !hand.joints.is_empty() {
        for j in &hand.joints {
            if !j.valid {
                continue;
            }
            let size = if j.name.contains("tip") {
                Vec3::splat(0.010)
            } else if j.name == "palm" || j.name == "wrist" {
                Vec3::new(0.035, 0.018, 0.045)
            } else {
                Vec3::splat(0.013)
            };
            let (pos, rot) = transform(j.pose.position(), j.pose.rotation());
            let mut c = Cuboid::solid(pos, size, color);
            c.rotation = rot;
            cuboids.push(c);
        }
    } else {
        if let Some(grip) = hand.grip {
            let (pos, rot) = transform(grip.position(), grip.rotation());
            let mut c = Cuboid::solid_and_wire(
                pos,
                Vec3::new(0.035, 0.035, 0.06),
                color,
                Color3(255, 255, 255, 200),
            );
            c.rotation = rot;
            cuboids.push(c);
        }
        if let Some(aim) = hand.aim {
            let (aim_pos, aim_rot) = transform(aim.position(), aim.rotation());
            let mut c = Cuboid::wireframe(aim_pos, Vec3::splat(0.02), color);
            c.rotation = aim_rot;
            cuboids.push(c);

            let dir = aim_rot * Vec3::new(0.0, 0.0, -1.0);
            for i in 1..6 {
                let t = i as f32 * 0.06;
                cuboids.push(Cuboid::solid(
                    aim_pos + dir * t,
                    Vec3::splat(0.008),
                    Color3(color.0, color.1, color.2, 200),
                ));
            }
        }
    }
}

pub fn ground_grid() -> Vec<Cuboid> {
    let mut grid = Vec::new();
    let extent = 3.0_f32;
    let step = 0.5_f32;
    let n = (extent * 2.0 / step) as i32;
    let color = Color3(70, 70, 85, 255);

    for i in -n / 2..=n / 2 {
        let x = i as f32 * step;
        grid.push(Cuboid::solid(
            Vec3::new(x, 0.0, 0.0),
            Vec3::new(0.004, 0.002, extent),
            color,
        ));
        grid.push(Cuboid::solid(
            Vec3::new(0.0, 0.0, x),
            Vec3::new(extent, 0.002, 0.004),
            color,
        ));
    }
    grid
}
