use space_soup_engine::{DebugPacket, HandSample};

pub fn left_panel_text(p: &DebugPacket) -> String {
    let mut s = String::new();

    s.push_str("== HEAD ==\n");
    push_pose(&mut s, "pos", p.head.position());
    push_quat(&mut s, "rot", p.head.rotation());
    s.push('\n');

    s.push_str("== LEFT HAND ==\n");
    push_hand(&mut s, &p.left_hand);
    s.push('\n');

    s.push_str("== LEFT HAND JOINTS ==\n");
    push_joints(&mut s, &p.left_hand);

    s
}

pub fn right_panel_text(p: &DebugPacket) -> String {
    let mut s = String::new();

    s.push_str("== RIGHT HAND ==\n");
    push_hand(&mut s, &p.right_hand);
    s.push('\n');

    s.push_str("== LOCOMOTION ==\n");
    s.push_str(&format!("mode:     {}\n", p.locomotion.mode));
    s.push_str(&format!(
        "offset:   ({:.3}, {:.3}, {:.3})\n",
        p.locomotion.player_offset[0], p.locomotion.player_offset[1], p.locomotion.player_offset[2],
    ));
    s.push_str(&format!("yaw:      {:.1} deg\n", p.locomotion.player_yaw_deg));
    s.push_str(&format!("aiming:   {}\n", p.locomotion.teleport_aiming));
    s.push('\n');

    s.push_str("== REMOTE SCENE (from headset) ==\n");
    s.push_str(&format!("name:     {}\n", p.scene.scene_name));
    s.push_str(&format!("objects:  {}\n", p.scene.object_count));
    s.push_str(&format!("cuboids:  {}\n", p.scene.render_cuboids));
    s.push_str(&format!("meshes:   {}\n", p.scene.render_meshes));
    if !p.scene.active_animations.is_empty() {
        s.push_str(&format!("anims:    {}\n", p.scene.active_animations.join(", ")));
    }
    s.push('\n');

    s.push_str("== TIMING (headset) ==\n");
    s.push_str(&format!("dt:       {:.4}s\n", p.timing.dt_seconds));
    s.push_str(&format!("fps:      {:.1}\n", p.timing.fps));
    s.push_str(&format!("frame:    {}\n", p.timing.frame_count));

    if !p.log_lines.is_empty() {
        s.push_str("\n== LOG ==\n");
        for line in p.log_lines.iter().rev().take(10) {
            s.push_str(line);
            s.push('\n');
        }
    }

    s
}

fn push_pose(s: &mut String, label: &str, pos: glam::Vec3) {
    s.push_str(&format!("{label}:      ({:.3}, {:.3}, {:.3})\n", pos.x, pos.y, pos.z));
}

fn push_quat(s: &mut String, label: &str, rot: glam::Quat) {
    s.push_str(&format!(
        "{label}:      ({:.3}, {:.3}, {:.3}, {:.3})\n",
        rot.x, rot.y, rot.z, rot.w,
    ));
}

fn push_hand(s: &mut String, hand: &HandSample) {
    s.push_str(&format!("tracking: {}\n", hand.tracking_active));
    if let Some(grip) = hand.grip {
        push_pose(s, "grip", grip.position());
    }
    if let Some(aim) = hand.aim {
        push_pose(s, "aim ", aim.position());
    }
    s.push_str(&format!("trigger:  {:.2}\n", hand.trigger));
    s.push_str(&format!("squeeze:  {:.2}\n", hand.squeeze));
    s.push_str(&format!("stick:    ({:.2}, {:.2})\n", hand.stick[0], hand.stick[1]));
    if hand.stick_click { s.push_str("stick_click: true\n"); }
    if hand.btn_a { s.push_str("A pressed\n"); }
    if hand.btn_b { s.push_str("B pressed\n"); }
    if hand.btn_x { s.push_str("X pressed\n"); }
    if hand.btn_y { s.push_str("Y pressed\n"); }
}

fn push_joints(s: &mut String, hand: &HandSample) {
    if !hand.tracking_active || hand.joints.is_empty() {
        s.push_str("(hand tracking inactive)\n");
        return;
    }
    for j in &hand.joints {
        if !j.valid { continue; }
        let p = j.pose.position();
        s.push_str(&format!("{:14} ({:.3}, {:.3}, {:.3})\n", j.name, p.x, p.y, p.z));
    }
}
