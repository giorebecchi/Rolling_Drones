use bevy::prelude::*;
use std::collections::HashMap;
use bevy::sprite::Anchor;
use wg_2024::network::NodeId;
use crate::gui::login_window::{DisplayableLog, NodeConfig, NodesConfig};

pub struct RouteHighlightPlugin<S: States> {
    pub game_state: S,
}

impl<S: States> Plugin for RouteHighlightPlugin<S> {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<RouteHighlightState>()
            .init_resource::<ConnectionEntities>()
            .init_resource::<ConnectionUpdateQueue>()
            .add_systems(OnEnter(self.game_state.clone()), setup_connections)
            .add_systems(OnExit(self.game_state.clone()), cleanup_connections)
            .add_systems(Update, (
                process_connection_updates,
                update_route_highlights,
                fade_route_highlights,
            ).chain().run_if(in_state(self.game_state.clone())));
    }
}

#[derive(Component)]
struct Connection {}

#[derive(Component)]
struct RouteHighlight {
    timer: Timer
}

#[derive(Resource, Default)]
struct ConnectionEntities {
    connections: HashMap<(NodeId, NodeId), Entity>,
}

#[derive(Resource, Default)]
struct RouteHighlightState {
    last_route_count: HashMap<(NodeId, u64), usize>,
    color_index: usize,
}

#[derive(Clone)]
pub enum ConnectionUpdate {
    Add { from: NodeId, to: NodeId },
    Remove { from: NodeId, to: NodeId },
    RemoveAllForNode { node: NodeId },
}
#[derive(Resource, Default)]
pub struct ConnectionUpdateQueue {
    updates: Vec<ConnectionUpdate>,
}

impl ConnectionUpdateQueue {
    pub fn add_connection(&mut self, from: NodeId, to: NodeId) {
        self.updates.push(ConnectionUpdate::Add { from, to });
    }

    pub fn remove_connection(&mut self, from: NodeId, to: NodeId) {
        self.updates.push(ConnectionUpdate::Remove { from, to });
    }

    pub fn remove_all_connections_for_node(&mut self, node: NodeId) {
        self.updates.push(ConnectionUpdate::RemoveAllForNode { node });
    }
}

const ROUTE_COLORS: [Color; 8] = [
    Color::srgb(1.0, 0.0, 0.0),    // Red
    Color::srgb(0.0, 1.0, 0.0),    // Green
    Color::srgb(0.0, 0.0, 1.0),    // Blue
    Color::srgb(1.0, 1.0, 0.0),    // Yellow
    Color::srgb(1.0, 0.0, 1.0),    // Magenta
    Color::srgb(0.0, 1.0, 1.0),    // Cyan
    Color::srgb(1.0, 0.5, 0.0),    // Orange
    Color::srgb(0.5, 0.0, 1.0),    // Purple
];

fn setup_connections(
    node_data: Res<NodesConfig>,
    mut commands: Commands,
    mut connection_entities: ResMut<ConnectionEntities>,
) {
    for node in &node_data.0 {
        for connected_id in &node.connected_node_ids {
            if let Some(connected_node) = node_data.0.iter().find(|n| n.id == *connected_id) {
                if node.id < *connected_id {
                    spawn_connection(&mut commands, &mut connection_entities, node, connected_node);
                }
            }
        }
    }
}

fn cleanup_connections(
    mut commands: Commands,
    connection_query: Query<Entity, With<Connection>>,
    mut connection_entities: ResMut<ConnectionEntities>,
    mut highlight_state: ResMut<RouteHighlightState>,
) {
    for entity in &connection_query {
        commands.entity(entity).despawn();
    }

    connection_entities.connections.clear();
    highlight_state.last_route_count.clear();
    highlight_state.color_index = 0;
}

fn spawn_connection(
    commands: &mut Commands,
    connection_entities: &mut ResMut<ConnectionEntities>,
    from_node: &NodeConfig,
    to_node: &NodeConfig,
) {
    let start = Vec2::new(from_node.position[0], from_node.position[1]);
    let end = Vec2::new(to_node.position[0], to_node.position[1]);

    let diff = end - start;
    let length = diff.length();
    let angle = diff.y.atan2(diff.x);

    let entity = commands.spawn((
        Sprite {
            color: Color::WHITE,
            custom_size: Some(Vec2::new(length, 2.0)),
            anchor: Anchor::CenterLeft,
            ..default()
        },
        Transform::from_translation(Vec3::new(start.x, start.y, -1.0))
            .with_rotation(Quat::from_rotation_z(angle)),
        Connection{}
    )).id();

    connection_entities.connections.insert((from_node.id, to_node.id), entity);
    connection_entities.connections.insert((to_node.id, from_node.id), entity);
}

fn process_connection_updates(
    mut update_queue: ResMut<ConnectionUpdateQueue>,
    mut connection_entities: ResMut<ConnectionEntities>,
    mut commands: Commands,
    mut node_data: ResMut<NodesConfig>
) {
    for update in update_queue.updates.drain(..) {
        match update {
            ConnectionUpdate::Add { from, to } => {
                if connection_entities.connections.contains_key(&(from, to)) {
                    continue;
                }

                let mut from_exists = false;
                let mut to_exists = false;

                for node in &mut node_data.0 {
                    if node.id == from {
                        from_exists = true;
                        if !node.connected_node_ids.contains(&to) {
                            node.connected_node_ids.push(to);
                        }
                    }
                    if node.id == to {
                        to_exists = true;
                        if !node.connected_node_ids.contains(&from) {
                            node.connected_node_ids.push(from);
                        }
                    }
                }

                if from_exists && to_exists {
                    let from_node = node_data.0.iter().find(|n| n.id == from).cloned();
                    let to_node = node_data.0.iter().find(|n| n.id == to).cloned();

                    if let (Some(from_node), Some(to_node)) = (from_node, to_node) {

                        if from < to {
                            spawn_connection(&mut commands, &mut connection_entities, &from_node, &to_node);
                        } else {
                            spawn_connection(&mut commands, &mut connection_entities, &to_node, &from_node);
                        }
                    }
                }
            }

            ConnectionUpdate::Remove { from, to } => {
                for node in &mut node_data.0 {
                    if node.id == from {
                        node.connected_node_ids.retain(|&id| id != to);
                    }
                    if node.id == to {
                        node.connected_node_ids.retain(|&id| id != from);
                    }
                }

                if let Some(&entity) = connection_entities.connections.get(&(from, to)) {
                    commands.entity(entity).despawn();
                    connection_entities.connections.remove(&(from, to));
                    connection_entities.connections.remove(&(to, from));
                }
            }

            ConnectionUpdate::RemoveAllForNode { node } => {
                let mut connections_to_remove = Vec::new();
                for ((from, to), &entity) in &connection_entities.connections {
                    if *from == node || *to == node {
                        connections_to_remove.push((*from, *to, entity));
                    }
                }

                connections_to_remove.sort_by_key(|(from, to, entity)| (*entity, *from.min(to), *from.max(to)));
                connections_to_remove.dedup_by_key(|(_, _, entity)| *entity);


                for node_config in &mut node_data.0 {
                    if node_config.id == node {
                        node_config.connected_node_ids.clear();
                    } else {
                        node_config.connected_node_ids.retain(|&id| id != node);
                    }
                }

                for (from, to, entity) in connections_to_remove {
                    commands.entity(entity).despawn();
                    connection_entities.connections.remove(&(from, to));
                    connection_entities.connections.remove(&(to, from));
                }
            }
        }
    }
}

fn update_route_highlights(
    displayable_log: Res<DisplayableLog>,
    mut highlight_state: ResMut<RouteHighlightState>,
    connection_entities: Res<ConnectionEntities>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Sprite), With<Connection>>,
) {
    let current_routes: std::collections::HashSet<(NodeId, u64)> =
        displayable_log.route_attempt.keys().cloned().collect();

    highlight_state.last_route_count.retain(|key, _| current_routes.contains(key));

    let mut active_connections = std::collections::HashSet::new();
    for (_, routes) in &displayable_log.route_attempt {
        for route in routes {
            for window in route.windows(2) {
                if let [from, to] = window {
                    active_connections.insert((*from, *to));
                    active_connections.insert((*to, *from));
                }
            }
        }
    }

    for ((from, to), &entity) in &connection_entities.connections {
        // Skip duplicate entries (connections are stored bidirectionally)
        if from > to {
            continue;
        }

        if !active_connections.contains(&(*from, *to)) {
            if let Ok((_, mut sprite)) = query.get_mut(entity) {
                sprite.color = Color::WHITE;
                commands.entity(entity).remove::<RouteHighlight>();
            }
        }
    }

    for ((node_id, msg_id), routes) in &displayable_log.route_attempt {
        let key = (*node_id, *msg_id);
        let current_count = routes.len();
        let last_count = highlight_state.last_route_count.get(&key).copied().unwrap_or(0);

        if current_count > last_count {
            for route in routes.iter().skip(last_count) {
                let color = ROUTE_COLORS[highlight_state.color_index % ROUTE_COLORS.len()];
                highlight_state.color_index += 1;

                for window in route.windows(2) {
                    if let [from, to] = window {
                        if let Some(&entity) = connection_entities.connections.get(&(*from, *to)) {
                            if let Ok((_, mut sprite)) = query.get_mut(entity) {
                                sprite.color = color;

                                commands.entity(entity).insert(RouteHighlight {
                                    timer: Timer::from_seconds(5.0, TimerMode::Once)
                                });
                            }
                        }
                    }
                }
            }

            highlight_state.last_route_count.insert(key, current_count);
        }
    }
}
fn fade_route_highlights(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Sprite, &mut RouteHighlight)>,
) {
    for (entity, mut sprite, mut highlight) in &mut query {
        highlight.timer.tick(time.delta());

        if highlight.timer.finished() {
            sprite.color = Color::WHITE;
            commands.entity(entity).remove::<RouteHighlight>();
        }
    }
}

