//! Immutable service records and atomic realm publication.
use super::super::game_services as api;
use super::*;
use api::{Action, Group, Id, Item, Kind, Offer, Outcome, Status};
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Member {
    character: u64,
    party: Option<Id>,
    guild: Option<Id>,
    invites: Vec<Id>,
    items: Vec<Id>,
    offers: Vec<Id>,
}
impl Member {
    fn group(&self, kind: Kind) -> Option<Id> {
        match kind {
            Kind::Party => self.party,
            Kind::Guild => self.guild,
        }
    }
    fn set_group(&mut self, kind: Kind, id: Option<Id>) {
        match kind {
            Kind::Party => self.party = id,
            Kind::Guild => self.guild = id,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "service", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Record {
    Member(Member),
    Group(Group),
    Item(Item),
    Offer(Offer),
    Receipt(api::Receipt),
    Loot(api::LootReceipt),
}
fn key(domain: &[u8], bytes: &[u8]) -> Id {
    let mut value = b"verse.realm.services.v1\0".to_vec();
    value.extend_from_slice(domain);
    value.push(0);
    value.extend_from_slice(bytes);
    disk::digest(&value)
}
fn receipt_key(character: u64, operation: [u8; 16]) -> Id {
    let mut data = character.to_be_bytes().to_vec();
    data.extend_from_slice(&operation);
    key(b"receipt", &data)
}
impl Record {
    pub(super) fn key(&self) -> Id {
        match self {
            Self::Member(m) => key(b"member", &m.character.to_be_bytes()),
            Self::Group(g) => key(b"group", &g.id),
            Self::Item(i) => key(b"item", &i.id),
            Self::Offer(o) => key(b"offer", &o.id),
            Self::Receipt(r) => receipt_key(r.character, r.operation),
            Self::Loot(r) => key(b"loot", &r.grant.event),
        }
    }
}
fn ordered<T: Ord>(values: &[T], cap: usize) -> bool {
    values.len() <= cap && values.windows(2).all(|w| w[0] < w[1])
}
fn insert<T: Ord>(values: &mut Vec<T>, value: T, cap: usize) -> Result<(), String> {
    if let Err(index) = values.binary_search(&value) {
        if values.len() >= cap {
            return Err("Service membership budget exceeded".into());
        }
        values.insert(index, value);
    }
    Ok(())
}
fn remove<T: PartialEq>(values: &mut Vec<T>, value: &T) {
    values.retain(|v| v != value);
}
struct Plan {
    records: BTreeMap<Id, Record>,
    games: BTreeMap<u64, Gateway>,
}
impl Plan {
    fn new() -> Self {
        Self {
            records: BTreeMap::new(),
            games: BTreeMap::new(),
        }
    }
    fn put(&mut self, r: Record) {
        self.records.insert(r.key(), r);
    }
}
impl Realm {
    fn service_record(&self, id: Id) -> Result<Option<Record>, String> {
        match registry::get(self, self.manifest.registry_root, id)? {
            Some(registry::Record::Service(r)) if r.key() == id => Ok(Some(r)),
            None => Ok(None),
            _ => Err("Realm service record identity differs".into()),
        }
    }
    fn member(&self, character: u64) -> Result<Member, String> {
        match self.service_record(key(b"member", &character.to_be_bytes()))? {
            Some(Record::Member(m))
                if ordered(&m.invites, 16) && ordered(&m.items, 64) && ordered(&m.offers, 16) =>
            {
                Ok(m)
            }
            None => Ok(Member {
                character,
                ..Default::default()
            }),
            _ => Err("Invalid realm membership".into()),
        }
    }
    fn group(&self, id: Id) -> Result<Group, String> {
        match self.service_record(key(b"group", &id))? {
            Some(Record::Group(g))
                if ordered(&g.members, g.kind.capacity())
                    && ordered(&g.invites, 64)
                    && (g.members.is_empty() && g.leader == 0 || g.members.contains(&g.leader)) =>
            {
                Ok(g)
            }
            _ => Err("Realm group is unavailable".into()),
        }
    }
    fn item_instance(&self, id: Id) -> Result<Item, String> {
        match self.service_record(key(b"item", &id))? {
            Some(Record::Item(i)) if i.owner != 0 && i.version != 0 => Ok(i),
            _ => Err("Item instance is unavailable".into()),
        }
    }
    fn offer(&self, id: Id) -> Result<Offer, String> {
        match self.service_record(key(b"offer", &id))? {
            Some(Record::Offer(o)) if o.give.len() <= 8 && o.want.len() <= 8 => Ok(o),
            _ => Err("Trade offer is unavailable".into()),
        }
    }
    fn service_character(
        &self,
        lease: &Lease,
        connection: ConnectionId,
        character: u64,
    ) -> Result<(), String> {
        let gateway = &self.games[&lease.instance];
        let principal = gateway.principal(connection)?.0;
        let admission = gateway.admission(connection)?;
        match self.owned(character, principal)?.residence {
            Residence::Resident { instance, actor }
                if instance == lease.instance
                    && actor == admission.actor().actor
                    && gateway.chamber.rewards.realm_character(actor) == Some(character) =>
            {
                Ok(())
            }
            _ => Err("Service character does not match the admitted connection".into()),
        }
    }
    fn service_resident(&self, character: u64, now: u64) -> Result<(u64, u64), String> {
        let placement = self
            .manifest
            .characters
            .get(&character)
            .ok_or("Service character is not resident")?;
        let slot = &self.manifest.instances[&placement.instance];
        if slot.phase == Phase::Stopped || slot.owner.is_none() || slot.expires_ms <= now {
            return Err("Service character authority is unavailable".into());
        }
        Ok((placement.instance, placement.actor))
    }
    fn service_commit(&mut self, mut plan: Plan, instances: &[u64]) -> Result<(), String> {
        for (instance, gateway) in &plan.games {
            super::super::save::decode_with_history(
                &gateway.checkpoint()?,
                gateway.content().ok_or("Service content is missing")?,
                *instance,
                Some(self.history.clone()),
            )?;
        }
        let root = registry::put(
            self,
            self.manifest.registry_root,
            plan.records
                .into_values()
                .map(registry::Record::Service)
                .collect(),
        )?;
        for (instance, game) in std::mem::take(&mut plan.games) {
            self.games.insert(instance, game);
        }
        self.manifest.registry_root = root;
        self.services_commit = true;
        let result = self.publish(instances);
        self.services_commit = false;
        result
    }
    pub fn services_view(
        &mut self,
        lease: &Lease,
        connection: ConnectionId,
        character: u64,
        now: u64,
    ) -> Result<api::View, String> {
        self.check(lease, now)?;
        self.service_character(lease, connection, character)?;
        let m = self.member(character)?;
        let view = api::View {
            realm: self.manifest.id,
            character,
            groups: [m.party, m.guild]
                .into_iter()
                .flatten()
                .map(|id| self.group(id))
                .collect::<Result<_, _>>()?,
            invitations: m
                .invites
                .into_iter()
                .map(|id| self.group(id))
                .collect::<Result<_, _>>()?,
            items: m
                .items
                .into_iter()
                .map(|id| self.item_instance(id))
                .collect::<Result<_, _>>()?,
            offers: m
                .offers
                .into_iter()
                .map(|id| self.offer(id))
                .collect::<Result<_, _>>()?,
        };
        view.validate()?;
        Ok(view)
    }
    pub fn services_action(
        &mut self,
        lease: &Lease,
        connection: ConnectionId,
        realm: Id,
        character: u64,
        operation: [u8; 16],
        action: Action,
        now: u64,
    ) -> Result<api::Receipt, String> {
        self.check(lease, now)?;
        self.service_character(lease, connection, character)?;
        if realm != self.manifest.id || operation == [0; 16] {
            return Err("Service realm or operation identity is incompatible".into());
        }
        let digest =
            disk::digest(&serde_json::to_vec(&action).map_err(|_| "Cannot encode service action")?);
        if let Some(Record::Receipt(r)) = self.service_record(receipt_key(character, operation))? {
            return if r.realm == realm && r.digest == digest {
                Ok(r)
            } else {
                Err("Service operation already binds a different action".into())
            };
        }
        let mut identity = realm.to_vec();
        identity.extend_from_slice(&character.to_be_bytes());
        identity.extend_from_slice(&operation);
        let id = key(b"identity", &identity);
        let mut plan = Plan::new();
        let outcome = match action {
            Action::Create { kind, name } => {
                if name.trim().is_empty()
                    || name.len() > 80
                    || !name.bytes().all(|b| (32..=126).contains(&b))
                {
                    return Err("Invalid group name".into());
                }
                let mut m = self.member(character)?;
                if m.group(kind).is_some() {
                    return Err("Character already belongs to this group kind".into());
                }
                let g = Group {
                    id,
                    kind,
                    name,
                    leader: character,
                    members: vec![character],
                    invites: vec![],
                    revision: 1,
                };
                m.set_group(kind, Some(id));
                plan.put(Record::Member(m));
                plan.put(Record::Group(g.clone()));
                Outcome::Group { group: g }
            }
            Action::Invite { group, target } => {
                self.contact(character, target)?;
                self.character(target)?;
                let mut g = self.group(group)?;
                let mut m = self.member(target)?;
                if g.leader != character || g.members.contains(&target) || m.group(g.kind).is_some()
                {
                    return Err("Group invitation is unauthorized or unavailable".into());
                }
                insert(&mut g.invites, target, 64)?;
                insert(&mut m.invites, group, 16)?;
                g.revision = g
                    .revision
                    .checked_add(1)
                    .ok_or("Group revisions exhausted")?;
                plan.put(Record::Member(m));
                plan.put(Record::Group(g.clone()));
                Outcome::Group { group: g }
            }
            Action::Join { group } => {
                let mut g = self.group(group)?;
                self.contact(character, g.leader)?;
                let mut m = self.member(character)?;
                if !g.invites.contains(&character)
                    || !m.invites.contains(&group)
                    || m.group(g.kind).is_some()
                    || g.leader == 0
                {
                    return Err("Group join requires a current invitation".into());
                }
                insert(&mut g.members, character, g.kind.capacity())?;
                remove(&mut g.invites, &character);
                remove(&mut m.invites, &group);
                m.set_group(g.kind, Some(group));
                g.revision = g
                    .revision
                    .checked_add(1)
                    .ok_or("Group revisions exhausted")?;
                plan.put(Record::Member(m));
                plan.put(Record::Group(g.clone()));
                Outcome::Group { group: g }
            }
            Action::Decline { group } => {
                let mut g = self.group(group)?;
                let mut m = self.member(character)?;
                if !g.invites.contains(&character) || !m.invites.contains(&group) {
                    return Err("Group invitation is unavailable".into());
                }
                remove(&mut g.invites, &character);
                remove(&mut m.invites, &group);
                g.revision = g
                    .revision
                    .checked_add(1)
                    .ok_or("Group revisions exhausted")?;
                plan.put(Record::Member(m));
                plan.put(Record::Group(g.clone()));
                Outcome::Group { group: g }
            }
            Action::Leave { group } | Action::Remove { group, target: _ } => {
                let target = match action {
                    Action::Remove { target, .. } => target,
                    _ => character,
                };
                let mut g = self.group(group)?;
                let mut m = self.member(target)?;
                if !g.members.contains(&target)
                    || m.group(g.kind) != Some(group)
                    || (target != character && g.leader != character)
                {
                    return Err("Group removal is unauthorized".into());
                }
                remove(&mut g.members, &target);
                m.set_group(g.kind, None);
                if g.leader == target {
                    g.leader = g.members.first().copied().unwrap_or(0);
                }
                if g.members.is_empty() {
                    for invited in std::mem::take(&mut g.invites) {
                        let mut member = self.member(invited)?;
                        remove(&mut member.invites, &group);
                        plan.put(Record::Member(member));
                    }
                }
                g.revision = g
                    .revision
                    .checked_add(1)
                    .ok_or("Group revisions exhausted")?;
                plan.put(Record::Member(m));
                plan.put(Record::Group(g.clone()));
                Outcome::Group { group: g }
            }
            Action::Materialize { definition } => {
                let (instance, actor) = self.service_resident(character, now)?;
                let game = &self.games[&instance];
                let gear = game.equipment().item(definition)?;
                let mut m = self.member(character)?;
                let allocated = m
                    .items
                    .iter()
                    .map(|id| self.item_instance(*id))
                    .collect::<Result<Vec<_>, _>>()?
                    .iter()
                    .filter(|i| i.definition == definition)
                    .count();
                let owned = game
                    .character_rewards(actor)
                    .and_then(|c| c.items.get(&definition))
                    .copied()
                    .unwrap_or(0);
                if allocated >= owned as usize {
                    return Err("No unallocated owned gear remains".into());
                }
                let item = Item {
                    id,
                    owner: character,
                    definition,
                    catalog: disk::digest(
                        &serde_json::to_vec(gear).map_err(|_| "Cannot encode gear definition")?,
                    ),
                    version: 1,
                    locked: None,
                };
                insert(&mut m.items, id, 64)?;
                plan.put(Record::Member(m));
                plan.put(Record::Item(item.clone()));
                Outcome::Item { item }
            }
            Action::Offer {
                to,
                give,
                want,
                expires_ms,
            } => {
                if to == character
                    || expires_ms <= now
                    || expires_ms > now.saturating_add(300_000)
                    || give.is_empty() && want.is_empty()
                {
                    return Err("Invalid trade recipient, deadline, or empty exchange".into());
                }
                self.contact(character, to)?;
                self.service_resident(to, now)?;
                let offer = Offer {
                    id,
                    from: character,
                    to,
                    give,
                    want,
                    expires_ms,
                    status: Status::Offered,
                };
                for (owner, tokens) in [(character, &offer.give), (to, &offer.want)] {
                    if tokens.len() > 8 || !tokens.windows(2).all(|w| w[0].id < w[1].id) {
                        return Err("Trade items require bounded sorted unique identities".into());
                    }
                    for token in tokens {
                        let mut item = self.item_instance(token.id)?;
                        if item.owner != owner
                            || item.version != token.version
                            || item.locked.is_some()
                        {
                            return Err("Trade item ownership, version, or lock is stale".into());
                        }
                        if owner == character {
                            item.locked = Some(id);
                            plan.put(Record::Item(item));
                        }
                    }
                    let mut m = self.member(owner)?;
                    insert(&mut m.offers, id, 16)?;
                    plan.put(Record::Member(m));
                }
                plan.put(Record::Offer(offer.clone()));
                Outcome::Trade { offer }
            }
            Action::Accept { offer } | Action::Cancel { offer } => {
                let mut offer = self.offer(offer)?;
                let accept = matches!(action, Action::Accept { .. });
                if offer.status != Status::Offered
                    || (accept && (offer.to != character || now >= offer.expires_ms))
                    || (!accept && offer.from != character && offer.to != character)
                {
                    return Err("Trade consent is unauthorized, expired, or already settled".into());
                }
                if accept {
                    self.contact(offer.from, offer.to)?;
                }
                self.settle_trade(&mut plan, &offer, accept, now)?;
                offer.status = if accept {
                    Status::Accepted
                } else {
                    Status::Cancelled
                };
                plan.put(Record::Offer(offer.clone()));
                Outcome::Trade { offer }
            }
        };
        let receipt = api::Receipt {
            realm,
            character,
            operation,
            digest,
            outcome,
        };
        plan.put(Record::Receipt(receipt.clone()));
        let mut instances = plan.games.keys().copied().collect::<Vec<_>>();
        if !instances.contains(&lease.instance) {
            instances.push(lease.instance);
        }
        self.service_commit(plan, &instances)?;
        Ok(receipt)
    }
}
impl Realm {
    fn settle_trade(
        &self,
        plan: &mut Plan,
        offer: &Offer,
        accept: bool,
        now: u64,
    ) -> Result<(), String> {
        let mut from = self.member(offer.from)?;
        let mut to = self.member(offer.to)?;
        if !from.offers.contains(&offer.id) || !to.offers.contains(&offer.id) {
            return Err("Trade participants lost the offer".into());
        }
        let mut giving = Vec::new();
        let mut wanting = Vec::new();
        for (owner, tokens, items) in [
            (offer.from, &offer.give, &mut giving),
            (offer.to, &offer.want, &mut wanting),
        ] {
            if !accept && owner == offer.to {
                continue;
            }
            for token in tokens {
                let item = self.item_instance(token.id)?;
                let expected_lock = if owner == offer.from {
                    Some(offer.id)
                } else {
                    None
                };
                if item.owner != owner
                    || item.version != token.version
                    || item.locked != expected_lock
                {
                    return Err("Trade item is stale or belongs to another offer".into());
                }
                items.push(item);
            }
        }
        for (owner, member, other, outgoing, incoming) in [
            (offer.from, &mut from, offer.to, &giving, &wanting),
            (offer.to, &mut to, offer.from, &wanting, &giving),
        ] {
            remove(&mut member.offers, &offer.id);
            if accept {
                let (instance, actor) = self.service_resident(owner, now)?;
                let game = plan
                    .games
                    .entry(instance)
                    .or_insert_with(|| self.games[&instance].fork());
                for item in incoming {
                    let gear = game.equipment().item(item.definition)?;
                    if disk::digest(
                        &serde_json::to_vec(gear).map_err(|_| "Cannot encode gear definition")?,
                    ) != item.catalog
                    {
                        return Err(
                            "Destination gear definition differs from the item instance".into()
                        );
                    }
                }
                let entries = |items: &[Item]| {
                    let mut counts = BTreeMap::<u64, u32>::new();
                    for item in items {
                        *counts.entry(item.definition).or_default() += 1;
                    }
                    counts
                        .into_iter()
                        .map(|(id, count)| super::super::rewards::Entry { id, count })
                        .collect::<Vec<_>>()
                };
                let mut source = [0; 32];
                source[..8].copy_from_slice(b"VTRADE01");
                source[8..].copy_from_slice(&offer.id[..24]);
                let tx = super::super::rewards::Transaction {
                    instance,
                    actor,
                    source,
                    experience: 0,
                    items: entries(incoming),
                    spent: entries(outgoing),
                    quests: vec![],
                    acceptance: None,
                    outfit: None,
                    equipment: None,
                };
                game.chamber.apply_progression(tx)?;
                for item in outgoing {
                    remove(&mut member.items, &item.id);
                }
                for item in incoming {
                    insert(&mut member.items, item.id, 64)?;
                }
            }
            for original in outgoing {
                let mut item = original.clone();
                item.locked = None;
                if accept {
                    item.owner = other;
                    item.version = item
                        .version
                        .checked_add(1)
                        .ok_or("Item versions exhausted")?;
                }
                plan.put(Record::Item(item));
            }
        }
        plan.put(Record::Member(from));
        plan.put(Record::Member(to));
        Ok(())
    }
    /// Publishes one authored party outcome and freezes its eligible recipients.
    pub fn party_loot(
        &mut self,
        lease: &Lease,
        grant: api::Loot,
        now: u64,
    ) -> Result<api::LootReceipt, String> {
        self.check(lease, now)?;
        if grant.event == [0; 32] || grant.items.len() > 8 || grant.quests.len() > 8 {
            return Err("Party loot requires a nonzero authoritative event".into());
        }
        if let Some(Record::Loot(receipt)) = self.service_record(key(b"loot", &grant.event))? {
            return if receipt.grant == grant {
                Ok(receipt)
            } else {
                Err("Party loot event already binds different recipients or amounts".into())
            };
        }
        let group = self.group(grant.party)?;
        if group.kind != Kind::Party {
            return Err("Party loot requires a party".into());
        }
        let recipients = group
            .members
            .into_iter()
            .filter(|id| self.manifest.characters.contains_key(id))
            .collect::<Vec<_>>();
        if recipients.is_empty() {
            return Err("Party loot has no eligible resident recipients".into());
        }
        let mut plan = Plan::new();
        let mut receipts = Vec::new();
        for character in &recipients {
            let (instance, actor) = self.service_resident(*character, now)?;
            let gateway = plan
                .games
                .entry(instance)
                .or_insert_with(|| self.games[&instance].fork());
            let mut source = [0; 32];
            source[..8].copy_from_slice(b"VPLOOT01");
            source[8..].copy_from_slice(&disk::digest(&grant.event)[..24]);
            receipts.push(gateway.grant_reward(super::super::rewards::Transaction {
                instance,
                actor,
                source,
                experience: grant.experience,
                items: grant.items.clone(),
                quests: grant.quests.clone(),
                acceptance: None,
                spent: vec![],
                outfit: None,
                equipment: None,
            })?);
        }
        let receipt = api::LootReceipt {
            grant,
            recipients,
            receipts,
        };
        plan.put(Record::Loot(receipt.clone()));
        let mut instances = plan.games.keys().copied().collect::<Vec<_>>();
        if !instances.contains(&lease.instance) {
            instances.push(lease.instance);
        }
        self.service_commit(plan, &instances)?;
        Ok(receipt)
    }
    pub(super) fn services_request(
        &mut self,
        lease: &Lease,
        connection: ConnectionId,
        now: u64,
        body: &Body,
    ) -> Option<Result<super::super::wire::Reply, String>> {
        use super::super::wire::Reply;
        match body {
            Body::Services { character } => Some(
                self.services_view(lease, connection, *character, now)
                    .map(|view| Reply::Services { view }),
            ),
            Body::ServiceAction {
                realm,
                character,
                operation,
                action,
            } => Some(
                self.services_action(
                    lease,
                    connection,
                    *realm,
                    *character,
                    *operation,
                    action.clone(),
                    now,
                )
                .map(|receipt| Reply::ServiceApplied { receipt }),
            ),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
