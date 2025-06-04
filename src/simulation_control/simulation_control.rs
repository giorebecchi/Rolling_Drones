
use crossbeam_channel::{select_biased, unbounded, Sender};
use std::collections::{HashMap, HashSet};
use wg_2024::controller::{DroneCommand,DroneEvent};
use wg_2024::drone::{Drone};
use wg_2024::network::{NodeId};
use wg_2024::packet::{Ack, Fragment, Packet, PacketType};
use petgraph::Graph;
use petgraph::graphmap::UnGraphMap;
use wg_2024::packet::PacketType::{FloodRequest, MsgFragment};
use crate::gui::login_window::{NodeType, SimulationController, SHARED_LOG};
use crate::gui::shared_info_plugin::SHARED_STATE;
use crate::common_things::common::{BackGroundFlood, ChatClientEvent, ChatEvent, ChatServerEvent, ClientType, CommandChat, ContentCommands, ContentRequest, ContentType, MediaServerEvent, RequestEvent, ServerCommands, ServerEvent, TextServerEvent, WebBrowserEvents};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Copy)]
pub enum MyNodeType{
    WebBrowser,
    ChatClient,
    TextServer,
    MediaServer,
    ChatServer
}


impl Default for SimulationController{
    fn default() -> Self {
        let (sender, receiver) = unbounded();
        let (_, chat_recv)=unbounded();
        let (_, web_recv)=unbounded();
        let (_, server_recv)=unbounded();
        Self {
            node_event_send: sender,
            node_event_recv: receiver,
            drones: HashMap::new(),
            packet_channel: HashMap::new(),
            neighbours: HashMap::new(),
            client:  HashMap::new(),
            web_client: HashMap::new(),
            text_server: HashMap::new(),
            media_server: HashMap::new(),
            chat_server: HashMap::new(),
            seen_floods: HashSet::new(),
            client_list: HashMap::new(),
            chat_event: chat_recv,
            web_event: web_recv,
            server_event: server_recv,
            messages: HashMap::new(),
            incoming_message: HashMap::new(),
            register_success : HashMap::new(),
            background_flooding: HashMap::new(),
            chat_active: true,
            web_active: true
        }
    }
}



impl SimulationController {
    pub(crate) fn run(&mut self) {
        let mut flood_req_hash = HashSet::new();
        let no_chat_client=crossbeam_channel::never();
        let no_web_browser=crossbeam_channel::never();

        loop {
            select_biased! {
                recv(if self.chat_active { &self.chat_event } else { &no_chat_client }) -> event => {
                    if let Ok(chat_event) = event {
                        self.handle_chat_event(chat_event);
                    }
                }
                recv(if self.web_active { &self.web_event } else { &no_web_browser }) -> event => {
                    if let Ok(web_event) = event {
                        self.handle_web_event(web_event);
                    }
                }
                recv(self.server_event) -> event => {
                    if let Ok(server_event) = event {
                        self.handle_server_event(server_event);
                    }
                }
                recv(self.node_event_recv) -> command => {
                    if let Ok(drone_event) = command {
                        self.handle_drone_event(drone_event, &mut flood_req_hash);
                    }
                }
            }
        }
    }

    fn handle_chat_event(&mut self, chat_event: ChatClientEvent) {
        match chat_event {
            ChatClientEvent::IncomingMessage((id_client, id_server, id_from), message) => {
                self.handle_incoming_message((id_client, id_server, id_from), message);
            }
            ChatClientEvent::ClientList((id_client, id_server), registered_clients) => {
                self.handle_client_list((id_client, id_server), registered_clients);
            }
            ChatClientEvent::RegisteredSuccess((id_client, id_server), result) => {
                self.handle_registration_success((id_client, id_server), result);
            }
            ChatClientEvent::ChatServers(client_id, chat_servers) => {
                self.handle_chat_servers(client_id, chat_servers);
            }
            ChatClientEvent::ClientType(client_type, node_id) => {
                self.handle_client_type(client_type, node_id);
            }
            ChatClientEvent::PacketInfo(client, event, session) => {
                self.handle_chat_packet_info(client, event, session);
            }
            ChatClientEvent::Graph(id, graph) => {
                self.handle_chat_graph(id, graph);
            }
            ChatClientEvent::InfoRequest(client, request_type, session) => {
                self.handle_chat_info_request(client, request_type, session);
            }
            _ => {}
        }
    }

    fn handle_incoming_message(&self, ids: (NodeId, NodeId, NodeId), message: String) {
        let (id_client, id_server, id_from) = ids;
        if let Ok(mut state) = SHARED_STATE.write() {
            if let Some(messages) = state.responses.get_mut(&(id_server, (id_from, id_client))) {
                messages.push(message.clone());
            } else {
                let mut messages = Vec::new();
                messages.push(message);
                state.responses.insert((id_server, (id_from, id_client)), messages);
            }
            state.is_updated = true;
        }
    }

    fn handle_client_list(&self, ids: (NodeId, NodeId), registered_clients: Vec<NodeId>) {
        let (id_client, id_server) = ids;
        if let Ok(mut state) = SHARED_STATE.write() {
            if let Some(current_clients) = state.client_list.get_mut(&(id_client, id_server)) {
                let _ = std::mem::replace(current_clients, registered_clients);
            } else {
                state.client_list.insert((id_client, id_server), registered_clients);
            }
            state.is_updated = true;
        }
    }

    fn handle_registration_success(&self, ids: (NodeId, NodeId), result: Result<(), String>) {
        let (id_client, id_server) = ids;
        if let Ok(mut state) = SHARED_STATE.write() {
            match result {
                Ok(_) => {
                    state.registered_clients.insert((id_client, id_server), true);
                }
                Err(_) => {
                    state.registered_clients.insert((id_client, id_server), false);
                }
            }
            state.is_updated = true;
        }
    }

    fn handle_chat_servers(&self, client_id: NodeId, chat_servers: Vec<NodeId>) {
        if let Ok(mut state) = SHARED_STATE.write() {
            if let Some(current_chat_servers) = state.chat_servers.get_mut(&client_id) {
                let _ = std::mem::replace(current_chat_servers, chat_servers);
            } else {
                state.chat_servers.insert(client_id, chat_servers);
            }
            state.is_updated = true;
        }
    }

    fn handle_client_type(&self, client_type: ClientType, node_id: NodeId) {
        if let Ok(mut state) = SHARED_STATE.write() {
            match client_type {
                ClientType::ChatClient => state.chat_clients.push(node_id),
                ClientType::WebBrowser => state.web_clients.push(node_id),
            }
            state.is_updated = true;
        }
    }

    fn handle_chat_packet_info(&self, client: NodeId, event: ChatEvent, session: u64) {
        let message = match event {
            ChatEvent::ChatServers(size) => {
                format!(
                    "Chat Client {}: received list of Chat Servers\nThe message was made of {} fragments\n",
                    client, size
                )
            }
            ChatEvent::ClientList(size) => {
                format!(
                    "Chat Client {}: received list of Chat Clients\nThe message was made of {} fragments\n",
                    client, size
                )
            }
            ChatEvent::IncomingMessage(size) => {
                format!(
                    "Chat Client {}: received a message\nThe message was made of {} fragments\n",
                    client, size
                )
            }
            ChatEvent::RegisteredSuccess(size) => {
                format!(
                    "Chat Client {}: registered successfully\nThe message was made of {} fragments\n",
                    client, size
                )
            }
            ChatEvent::ClientType(size) => {
                format!(
                    "Chat Client {}: revealed its type\nThe message was made of {} fragments\n",
                    client, size
                )
            }
        };

        if let Ok(mut state) = SHARED_LOG.write() {
            state.msg_log.insert((client, session), message);
            state.is_updated = true;
        }
    }

    fn handle_chat_graph(&self, id: NodeId, graph: UnGraphMap<NodeId, u32>) {
        if let Ok(mut state) = SHARED_LOG.write() {
            state.graph.insert(id, graph);
        }
    }

    fn handle_chat_info_request(&self, client: NodeId, request_type: RequestEvent, session: u64) {
        let message = match request_type {
            RequestEvent::AskType(size) => {
                format!(
                    "Chat Client {}: asked server type\nThe message was made of {} fragments\n",
                    client, size
                )
            }
            RequestEvent::Register(size) => {
                format!(
                    "Chat Client {}: made an attempt to register\nThe message was made of {} fragments\n",
                    client, size
                )
            }
            RequestEvent::GetList(size) => {
                format!(
                    "Chat Client {}: asked list of registered clients\nThe message was made of {} fragments\n",
                    client, size
                )
            }
            RequestEvent::SendMessage(size) => {
                format!(
                    "Chat Client {}: sent a text message\nThe message was made of {} fragments\n",
                    client, size
                )
            }
        };

        if let Ok(mut state) = SHARED_LOG.write() {
            state.msg_log.insert((client, session), message);
            state.is_updated = true;
        }
    }

    fn handle_web_event(&mut self, web_event: WebBrowserEvents) {
        match web_event {
            WebBrowserEvents::MediaServers(client, media_servers) => {
                self.handle_media_servers(client, media_servers);
            }
            WebBrowserEvents::TextServers(client, text_servers) => {
                self.handle_text_servers(client, text_servers);
            }
            WebBrowserEvents::ListFiles(client, media_paths) => {
                self.handle_list_files(client, media_paths);
            }
            WebBrowserEvents::MediaPosition(client, target_media_server) => {
                self.handle_media_position(client, target_media_server);
            }
            WebBrowserEvents::SavedMedia(client, actual_media) => {
                self.handle_saved_media(client, actual_media);
            }
            WebBrowserEvents::SavedTextFile(client, actual_file) => {
                self.handle_saved_text_file(client, actual_file);
            }
            WebBrowserEvents::PacketInfo(client, packet_info, session_id) => {
                self.handle_web_packet_info(client, packet_info, session_id);
            }
            WebBrowserEvents::Graph(id, graph) => {
                self.handle_web_graph(id, graph);
            }
            WebBrowserEvents::InfoRequest(client, request_type, session_id) => {
                self.handle_web_info_request(client, request_type, session_id);
            }
            _ => {}
        }
    }

    fn handle_media_servers(&self, client: NodeId, media_servers: Vec<NodeId>) {
        if let Ok(mut state) = SHARED_STATE.write() {
            if let Some(current_media_servers) = state.media_servers.get_mut(&client) {
                let _ = std::mem::replace(current_media_servers, media_servers);
            } else {
                state.media_servers.insert(client, media_servers);
            }
            state.is_updated = true;
        }
    }

    fn handle_text_servers(&self, client: NodeId, text_servers: Vec<NodeId>) {
        if let Ok(mut state) = SHARED_STATE.write() {
            if let Some(current_media_servers) = state.text_servers.get_mut(&client) {
                let _ = std::mem::replace(current_media_servers, text_servers);
            } else {
                state.text_servers.insert(client, text_servers);
            }
            state.is_updated = true;
        }
    }

    fn handle_list_files(&self, client: NodeId, media_paths: Vec<String>) {
        if let Ok(mut state) = SHARED_STATE.write() {
            if let Some(current_paths) = state.client_medias.get_mut(&client) {
                let _ = std::mem::replace(current_paths, media_paths);
            } else {
                state.client_medias.insert(client, media_paths);
            }
            state.is_updated = true;
        }
    }

    fn handle_media_position(&self, client: NodeId, target_media_server: NodeId) {
        if let Ok(mut state) = SHARED_STATE.write() {
            if let Some(current_media_server) = state.target_media_server.get_mut(&client) {
                let _ = std::mem::replace(current_media_server, target_media_server);
            } else {
                state.target_media_server.insert(client, target_media_server);
            }
            state.is_updated = true;
        }
    }

    fn handle_saved_media(&self, client: NodeId, actual_media: String) {
        if let Ok(mut state) = SHARED_STATE.write() {
            if let Some(current_path) = state.actual_media_path.get_mut(&client) {
                let _ = std::mem::replace(current_path, actual_media);
            } else {
                state.actual_media_path.insert(client, actual_media);
            }
            state.is_updated = true;
        }
    }

    fn handle_saved_text_file(&self, client: NodeId, actual_file: String) {
        if let Ok(mut state) = SHARED_STATE.write() {
            if let Some(current_path) = state.actual_file_path.get_mut(&client) {
                let _ = std::mem::replace(current_path, actual_file);
            } else {
                state.actual_file_path.insert(client, actual_file);
            }
            state.is_updated = true;
        }
    }

    fn handle_web_packet_info(&self, client: NodeId, packet_info: ContentType, session_id: u64) {
        let message = match packet_info {
            ContentType::TextServerList(size) => {
                format!(
                    "Web browser: {} received list of Text Servers\n the message was made of {} fragments\n",
                    client, size
                )
            }
            ContentType::MediaServerList(size) => {
                format!(
                    "Web browser: {} received list of Media Servers\n the message was made of {} fragments\n",
                    client, size
                )
            }
            ContentType::FileList(size) => {
                format!(
                    "Web browser: {} received File List\n the message was made of {} fragments\n",
                    client, size
                )
            }
            ContentType::MediaPosition(size) => {
                format!(
                    "Web browser: {} received Media Position\n the message was made of {} fragments\n",
                    client, size
                )
            }
            ContentType::SavedText(size) => {
                format!(
                    "Web browser: {} received a  Text File\n the message was made of {} fragments\n",
                    client, size
                )
            }
            ContentType::SavedMedia(size) => {
                format!(
                    "Web browser: {} received a Media\n the message was made of {} fragments\n",
                    client, size
                )
            }
        };

        if let Ok(mut state) = SHARED_LOG.write() {
            state.msg_log.insert((client, session_id), message);
            state.is_updated = true;
        }
    }

    fn handle_web_graph(&self, id: NodeId, graph: UnGraphMap<NodeId, u32>) {
        if let Ok(mut state) = SHARED_LOG.write() {
            state.graph.insert(id, graph);
        }
    }

    fn handle_web_info_request(&self, client: NodeId, request_type: ContentRequest, session_id: u64) {
        let message = match request_type {
            ContentRequest::GetText(size) => {
                format!(
                    "Web browser: {} asked for a text file\n the message was made of {} fragments\n",
                    client, size
                )
            }
            ContentRequest::AskTypes(size) => {
                format!(
                    "Web browser: {} asked server types\n the message was made of {} fragments\n",
                    client, size
                )
            }
            ContentRequest::GetList(size) => {
                format!(
                    "Web browser: {} asked for list of files\n the message was made of {} fragments\n",
                    client, size
                )
            }
            ContentRequest::GetPosition(size) => {
                format!(
                    "Web browser: {} asked for the position of a file\n the message was made of {} fragments\n",
                    client, size
                )
            }
            ContentRequest::GetMedia(size) => {
                format!(
                    "Web browser: {} asked for a media file\n the message was made of {} fragments\n",
                    client, size
                )
            }
        };

        if let Ok(mut state) = SHARED_LOG.write() {
            state.msg_log.insert((client, session_id), message);
            state.is_updated = true;
        }
    }

    fn handle_server_event(&mut self, server_event: ServerEvent) {
        match server_event {
            ServerEvent::Graph(id, graph) => {
                self.handle_server_graph(id, graph);
            }
            ServerEvent::TextPacketInfo(server_id, server_type, packet_info, session_id) => {
                self.handle_text_packet_info(server_id, server_type, packet_info, session_id);
            }
            ServerEvent::MediaPacketInfo(server_id, server_type, packet_info, session_id) => {
                self.handle_media_packet_info(server_id, server_type, packet_info, session_id);
            }
            ServerEvent::ChatPacketInfo(server_id, server_type, packet_info, session_id) => {
                self.handle_chat_packet_info_server(server_id, server_type, packet_info, session_id);
            }
        }
    }

    fn handle_server_graph(&self, id: NodeId, graph: Graph<(NodeId, wg_2024::packet::NodeType), f64>) {
        if let Ok(mut state) = SHARED_LOG.write() {
            state.server_graph.insert(id, graph);
            state.is_updated = true;
        }
    }

    fn handle_text_packet_info(&self, server_id: NodeId, server_type: MyNodeType, packet_info: TextServerEvent, session_id: u64) {
        let message = match packet_info {
            TextServerEvent::SendingServerTypeText(size) => {
                format!(
                    "{:?} {}: sent server type to client\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
            TextServerEvent::SendingServerTypeReq(size) => {
                format!(
                    "{:?} {}: asked server type to other servers\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
            TextServerEvent::SendingFileList(size) => {
                format!(
                    "{:?} {}: sent file list to client\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
            TextServerEvent::AskingForPathRes(size) => {
                format!(
                    "{:?} {}: asked media server for list of medias\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
            TextServerEvent::SendingPosition(size) => {
                format!(
                    "{:?} {}: sent media position to client\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
            TextServerEvent::SendingText(size) => {
                format!(
                    "{:?} {}: sent text to client\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
        };

        if let Ok(mut state) = SHARED_LOG.write() {
            state.msg_log.insert((server_id, session_id), message);
        }
    }

    fn handle_media_packet_info(&self, server_id: NodeId, server_type: MyNodeType, packet_info: MediaServerEvent, session_id: u64) {
        let message = match packet_info {
            MediaServerEvent::SendingServerTypeMedia(size) => {
                format!(
                    "{:?} {}: sent its type\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
            MediaServerEvent::SendingPathRes(size) => {
                format!(
                    "{:?} {}: sent its medias to text server\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
            MediaServerEvent::SendingMedia(size) => {
                format!(
                    "{:?} {}: sent media to client\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
        };

        if let Ok(mut state) = SHARED_LOG.write() {
            state.msg_log.insert((server_id, session_id), message);
        }
    }

    fn handle_chat_packet_info_server(&self, server_id: NodeId, server_type: MyNodeType, packet_info: ChatServerEvent, session_id: u64) {
        let message = match packet_info {
            ChatServerEvent::SendingServerTypeChat(size) => {
                format!(
                    "{:?} {}: sent its type to client\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
            ChatServerEvent::ClientRegistration(size) => {
                format!(
                    "{:?} {}: sent registration success to client\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
            ChatServerEvent::SendingClientList(size) => {
                format!(
                    "{:?} {}: sent registered client list to client\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
            ChatServerEvent::ForwardingMessage(size) => {
                format!(
                    "{:?} {}: forwarded a chat message\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
            ChatServerEvent::ClientElimination(size) => {
                format!(
                    "{:?} {}: unregistered a client\nthe message was made of {} fragments\n",
                    server_type, server_id, size
                )
            }
        };

        if let Ok(mut state) = SHARED_LOG.write() {
            state.msg_log.insert((server_id, session_id), message);
        }
    }

    fn handle_drone_event(&mut self, drone_event: DroneEvent, flood_req_hash: &mut HashSet<(NodeId, u64)>) {
        match drone_event {
            DroneEvent::PacketSent(ref packet) => {
                self.handle_packet_sent(packet, flood_req_hash);
            }
            DroneEvent::PacketDropped(ref packet) => {
                self.handle_packet_dropped(packet);
            }
            DroneEvent::ControllerShortcut(ref controller_shortcut) => {
                self.send_to_destination(controller_shortcut.clone());
            }
        }
    }

    fn handle_packet_sent(&self, packet: &Packet, flood_req_hash: &mut HashSet<(NodeId, u64)>) {
        match packet.pack_type.clone() {
            FloodRequest(flood_req) => {
                if flood_req_hash.insert((flood_req.initiator_id, flood_req.flood_id)) {
                    let node_type = self.determine_node_type(flood_req.initiator_id);

                    if let Ok(mut state) = SHARED_LOG.write() {
                        state.flooding_log.insert(
                            (node_type, flood_req.initiator_id),
                            format!(
                                "{:?} with id {} has initiated a flood with id {}\n",
                                flood_req.path_trace[0].1,
                                flood_req.initiator_id,
                                flood_req.flood_id
                            ),
                        );
                        state.is_updated = true;
                    }
                }
            }
            MsgFragment(_) => {
                self.handle_msg_fragment_sent(packet);
            }
            _=>{}
        }
    }

    fn determine_node_type(&self, node_id: NodeId) -> MyNodeType {
        if self.client.contains_key(&node_id) {
            MyNodeType::ChatClient
        } else if self.web_client.contains_key(&node_id) {
            MyNodeType::WebBrowser
        } else if self.text_server.contains_key(&node_id) {
            MyNodeType::TextServer
        } else if self.media_server.contains_key(&node_id) {
            MyNodeType::MediaServer
        } else {
            MyNodeType::ChatServer
        }
    }

    fn handle_msg_fragment_sent(&self, packet: &Packet) {
        if let Ok(mut state) = SHARED_LOG.write() {
            let initiator_node = packet.routing_header.hops[0];
            let session_id = packet.session_id;

            if let Some(routes) = state.route_attempt.get_mut(&(initiator_node, session_id)) {
                if !routes.contains(&packet.routing_header.hops) {
                    routes.push(packet.routing_header.hops.clone());
                }
            } else {
                let routes = vec![packet.routing_header.hops.clone()];
                state.route_attempt.insert((initiator_node, session_id), routes);
            }
            state.is_updated = true;
        }
    }

    fn handle_packet_dropped(&self, packet: &Packet) {
        let drone = packet.routing_header.hops[packet.routing_header.hop_index];

        match packet.pack_type.clone() {
            MsgFragment(fragment) => {
                self.handle_dropped_msg_fragment(drone, packet.session_id, fragment);
            }
            PacketType::Ack(ack) => {
                self.handle_dropped_ack(drone, packet.session_id, ack);
            }
            PacketType::Nack(nack) => {
                self.handle_dropped_nack(drone, packet.session_id, nack);
            }
            PacketType::FloodRequest(flood_req) => {
                self.handle_dropped_flood_request(drone, packet.session_id, flood_req);
            }
            PacketType::FloodResponse(flood_resp) => {
                self.handle_dropped_flood_response(drone, packet.session_id, flood_resp);
            }
        }
    }

    fn handle_dropped_msg_fragment(&self, drone: NodeId, session_id: u64, fragment: Fragment) {
        if let Ok(mut state) = SHARED_LOG.write() {
            if let Some(fragments) = state.lost_msg.get_mut(&(drone, session_id)) {
                fragments.push(fragment);
            } else {
                state.lost_msg.insert((drone, session_id), vec![fragment]);
            }
            state.is_updated = true;
        }
    }

    fn handle_dropped_ack(&self, drone: NodeId, session_id: u64, ack: Ack) {
        if let Ok(mut state) = SHARED_LOG.write() {
            if let Some(acks) = state.lost_ack.get_mut(&(drone, session_id)) {
                acks.push(ack);
            } else {
                state.lost_ack.insert((drone, session_id), vec![ack]);
            }
            state.is_updated = true;
        }
    }

    fn handle_dropped_nack(&self, drone: NodeId, session_id: u64, nack: wg_2024::packet::Nack) {
        if let Ok(mut state) = SHARED_LOG.write() {
            if let Some(nacks) = state.lost_nack.get_mut(&(drone, session_id)) {
                nacks.push(nack);
            } else {
                state.lost_nack.insert((drone, session_id), vec![nack]);
            }
            state.is_updated = true;
        }
    }

    fn handle_dropped_flood_request(&self, drone: NodeId, session_id: u64, flood_req: wg_2024::packet::FloodRequest) {
        if let Ok(mut state) = SHARED_LOG.write() {
            if let Some(requests) = state.lost_flood_req.get_mut(&(drone, session_id)) {
                requests.push(flood_req);
            } else {
                state.lost_flood_req.insert((drone, session_id), vec![flood_req]);
            }
            state.is_updated = true;
        }
    }

    fn handle_dropped_flood_response(&self, drone: NodeId, session_id: u64, flood_resp: wg_2024::packet::FloodResponse) {
        if let Ok(mut state) = SHARED_LOG.write() {
            if let Some(responses) = state.lost_flood_resp.get_mut(&(drone, session_id)) {
                responses.push(flood_resp);
            } else {
                state.lost_flood_resp.insert((drone, session_id), vec![flood_resp]);
            }
            state.is_updated = true;
        }
    }

    fn send_to_destination(&mut self, mut packet: Packet) {
        let addr = packet.routing_header.hops[packet.routing_header.hops.len() - 1];
        packet.routing_header.hop_index = packet.routing_header.hops.len()-1;

        if let Some(sender) = self.packet_channel.get(&addr) {
            sender.send(packet).unwrap();
        }else{
            println!("SC couldn't send to destination packet: {}",packet);
        }

    }
    pub fn ask_topology_graph(&self,node: NodeId, node_type: NodeType){
        match node_type {
            NodeType::ChatClient=> {
                if let Some(sender) = self.client.get(&node) {
                    sender.send(CommandChat::SendTopologyGraph).unwrap();
                }
            }
            NodeType::WebBrowser=>{
                if let Some(sender) = self.web_client.get(&node) {
                    sender.send(ContentCommands::SendTopologyGraph).unwrap();
                }
            }
            NodeType::ChatServer=>{
                if let Some(sender) = self.chat_server.get(&node){
                    sender.send(ServerCommands::SendTopologyGraph).unwrap();
                }
            }
            NodeType::TextServer=>{
                if let Some(sender) = self.text_server.get(&node){
                    sender.send(ServerCommands::SendTopologyGraph).unwrap();
                }

            }
            NodeType::MediaServer=>{
                if let Some(sender) = self.media_server.get(&node){
                    sender.send(ServerCommands::SendTopologyGraph).unwrap();
                }
            }
            _=>{
                println!("Tried to ask Topology Graph to unreachable node!");
            }
        }
    }




    pub fn crash_all(&mut self) {
        for (_, sender) in self.drones.iter() {
            sender.send(DroneCommand::Crash).unwrap();
        }
    }
    pub fn crash(&mut self, id: NodeId) {
        let nghb = self.neighbours.get(&id).unwrap();
        for neighbour in nghb.iter(){
            if let Some(sender) = self.drones.get(&neighbour) {
                sender.send(DroneCommand::RemoveSender(id)).unwrap();
            }
        }

        if let Some(drone_sender) = self.drones.get(&id) {
            if let Err(err) = drone_sender.send(DroneCommand::Crash) {
                println!("Failed to send Crash command to drone {}: {:?}", id, err);
            }
        } else {
            println!("No drone with ID {:?}", id);
        }

    }

    pub fn pdr(&mut self, id : NodeId, pdr: f32) {
        for (idd, sender) in self.drones.iter() {
            if idd == &id {
                sender.send(DroneCommand::SetPacketDropRate(pdr)).unwrap()
            }
        }
    }
    fn find_sender(&self, id: NodeId) -> Option<AnySender> {
        if let Some(drone_sender) = self.drones.get(&id) {
            Some(AnySender::Drone(drone_sender.clone()))
        } else if let Some(client_sender) = self.client.get(&id) {
            Some(AnySender::Client(client_sender.clone()))
        } else if let Some(web_sender) = self.web_client.get(&id) {
            Some(AnySender::Web(web_sender.clone()))
        } else if let Some(text_server_sender) = self.text_server.get(&id) {
            Some(AnySender::TextServer(text_server_sender.clone()))
        } else if let Some(media_server_sender) = self.media_server.get(&id) {
            Some(AnySender::MediaServer(media_server_sender.clone()))
        } else if let Some(chat_server_sender) = self.chat_server.get(&id) {
            Some(AnySender::ChatServer(chat_server_sender.clone()))
        } else {
            None
        }
    }
    fn send_add_sender_to_node(&self, from_id: NodeId, to_id: NodeId) {
        let sender = self.packet_channel.get(&from_id).unwrap().clone();

        match self.find_sender(to_id) {
            Some(node_sender) => {
                if let Err(err) = node_sender.send_add_sender_command(from_id, sender) {
                    println!("Failed to add sender {} to {}: {:?}", from_id, node_sender.sender_type(), err);
                }
            }
            None => println!("No sender found with ID {}", to_id),
        }
    }
    pub fn add_sender(&mut self, dst_id: NodeId, nghb_id: NodeId) {
        self.send_add_sender_to_node(dst_id, nghb_id);
        //(full duplex)
        self.send_add_sender_to_node(nghb_id, dst_id);
    }
    fn remove_sender_to_node(&self, from_id: NodeId, to_id: NodeId){
        match self.find_sender(from_id){
            Some(node_sender)=>{
                if let Err(err) = node_sender.send_remove_sender_command(to_id){
                    println!("Failed to remove sender {} from {}, {:?}", to_id, from_id, err);
                }
            },
            None=>println!("No sender found with ID {}", from_id),
        }
    }

    pub fn remove_sender(&mut self, dst_id: NodeId, nghb_id: NodeId) {
        self.remove_sender_to_node(dst_id, nghb_id);
        //(full duplex)
        self.remove_sender_to_node(nghb_id, dst_id);
    }
    fn ack(&mut self, mut packet: Packet) {
        let next_hop=packet.routing_header.hops[packet.routing_header.hop_index +1];
        if let Some(sender) = self.packet_channel.get(&next_hop) {
            packet.routing_header.hop_index+=1;
            sender.send(packet).unwrap();
        }else{
            println!("No sender found for hop {}", next_hop);
        }
    }
    fn msg_fragment(&mut self, mut packet: Packet){
        let next_hop=packet.routing_header.hops[packet.routing_header.hop_index+1];
        if let Some(sender) = self.packet_channel.get(&next_hop) {
            packet.routing_header.hop_index+=1;
            sender.send(packet).unwrap();
        }
    }
    pub fn initiate_flood(&self){
        for (_, sender) in self.background_flooding.iter(){
            sender.send(BackGroundFlood::Start).unwrap();
        }
    }
    pub fn send_message(&mut self, message: String, client_id: NodeId, destination_client: NodeId, chat_server: NodeId){
        self.client.get(&client_id).unwrap().send(CommandChat::SendMessage(destination_client, chat_server, message)).unwrap()
    }
    pub fn register_client(&mut self, client_id: NodeId, server_id: NodeId){
        self.client.get(&client_id).unwrap().send(CommandChat::RegisterClient(server_id)).unwrap();
    }
    pub fn get_client_list(&mut self, client_id: NodeId, server_id: NodeId){
        self.client.get(&client_id).unwrap().send(CommandChat::GetListClients(server_id)).unwrap();
    }
    pub fn get_chat_servers(&self){
        for (_,sender) in self.client.iter(){
            sender.send(CommandChat::SearchChatServers).unwrap();
        }
    }
    pub fn get_web_servers(&self){
        for (_,sender) in self.web_client.iter(){
            sender.send(ContentCommands::SearchTypeServers).unwrap()
        }
    }
    pub fn get_media_list(&self, web_browser: NodeId, text_server: NodeId){
        if let Some(sender)=self.web_client.get(&web_browser){
            sender.send(ContentCommands::GetTextList(text_server)).unwrap();
        }
    }
    pub fn get_text_file(&self, web_browser: NodeId, text_server: NodeId, text_file: String){
        if let Some(sender)=self.web_client.get(&web_browser){
            sender.send(ContentCommands::GetText(text_server, text_file)).unwrap();
        }
    }
    pub fn get_media_position(&self, web_browser: NodeId, text_server:NodeId, media_path: String){
        if let Some(sender)=self.web_client.get(&web_browser){
            sender.send(ContentCommands::GetMediaPosition(text_server, media_path)).unwrap();
        }
    }
    pub fn get_media_from(&self, web_browser: NodeId, media_server: NodeId, media_path: String){
        if let Some(sender)= self.web_client.get(&web_browser){
            sender.send(ContentCommands::GetMedia(media_server, media_path)).unwrap();
        }
    }
    pub fn change_in_topology(&self){
        for (_, sender) in self.client.iter(){
            sender.send(CommandChat::TopologyChanged).unwrap();
        }
        for (_, sender) in self.web_client.iter(){
            sender.send(ContentCommands::TopologyChanged).unwrap();
        }
        for (_, sender) in self.text_server.iter().chain(self.media_server.iter()).chain(self.chat_server.iter()){
            sender.send(ServerCommands::TopologyChanged).unwrap();
        }

    }

}
enum AnySender {
    Drone(Sender<DroneCommand>),
    Client(Sender<CommandChat>),
    Web(Sender<ContentCommands>),
    TextServer(Sender<ServerCommands>),
    MediaServer(Sender<ServerCommands>),
    ChatServer(Sender<ServerCommands>),
}
impl AnySender {
    fn send_add_sender_command(&self, dst_id: NodeId, sender: Sender<Packet>) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            AnySender::Drone(s) => Ok(s.send(DroneCommand::AddSender(dst_id, sender))?),
            AnySender::Client(s) => Ok(s.send(CommandChat::AddSender(dst_id, sender))?),
            AnySender::Web(s) => Ok(s.send(ContentCommands::AddSender(dst_id, sender))?),
            AnySender::TextServer(s) | AnySender::MediaServer(s) | AnySender::ChatServer(s) => {
                Ok(s.send(ServerCommands::AddSender(dst_id, sender))?)
            }
        }
    }

    fn send_remove_sender_command(&self, dst_id: NodeId) -> Result<(), Box<dyn std::error::Error>>{
        match self{
            AnySender::Drone(s)=> Ok(s.send(DroneCommand::RemoveSender(dst_id))?),
            AnySender::Client(s)=> Ok(s.send(CommandChat::RemoveSender(dst_id))?),
            AnySender::Web(s)=> Ok(s.send(ContentCommands::RemoveSender(dst_id))?),
            AnySender::TextServer(s) | AnySender::MediaServer(s) | AnySender::ChatServer(s)=> {
                Ok(s.send(ServerCommands::RemoveSender(dst_id))?)
            }
        }
    }

    fn sender_type(&self) -> &'static str {
        match self {
            AnySender::Drone(_) => "drone",
            AnySender::Client(_) => "client",
            AnySender::Web(_) => "web",
            AnySender::TextServer(_) => "text server",
            AnySender::MediaServer(_) => "media server",
            AnySender::ChatServer(_) => "chat server",
        }
    }
}
