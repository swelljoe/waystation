# The world outside the valley

Nothing here is ever shown to the player. It exists so that travellers who have
never met corroborate each other — so that the toll one of them paid in the north
is the toll another one used to collect, and the river that drowned a village in
the south is the river a third one is walking towards. A player who never leaves
the court should still be able to draw the map from what people say.

It is also the beginning of the brief a model would need if these lines are ever
generated rather than authored. See **If a model ever writes these**, below.

## What happened

The ash, and nobody knows what the ash was. War, or something falling, or a sky
that turned on its own — the game never explains it, and neither should a
traveller, because there is nobody left who could. Most people died. What is left
is barely habitable.

It happened **before living memory**. Not the speaker's, not the speaker's
parents'. Call it a hundred years if a number is needed, but no one alive has a
number, and the guesses people give each other are off by decades in both
directions. Some do not believe there was a before at all — that the world was
ever anything but this is a story old people tell, and a hard young person will
say so.

So no traveller has ever seen the world work. Nothing is remembered as lost. The
ash is not a catastrophe in anyone's past; it is the ground. What people talk
about instead is the arithmetic — how many were in the party when it set out,
how many meals are left, how many days the knees have in them.

Lives are shorter than they were, not because anyone knows what they were, but
because the ash takes its rent. The old are old at fifty.

Literacy went with everything else, a hundred years back, and did not come back.
A person who can read is rare enough that the Scribe's desk is not an ordinary
thing to walk in on.

## The valley

The waystation is a stone motel on the old road, at the edge of the ash, below
the mountain the Scribe came down from. The old road runs west to east through
the valley. It bends south to cross the river Sill at the ford, then climbs east
to the pass. A branch leaves it northward towards the Kiln.

A century has been through the lot and the ridge: the paving is broken up into
pieces a person can lift, and the orchard burnt long enough ago that the stumps
have gone soft. Whether anyone used the place in between is not known. Probably
somebody did, more than once, and left again.

A lit hearth here is the only advertisement, and a frightening one — see
`crates/game/src/visitors.rs` for why nobody comes for the first several nights.

## The building, which should not exist

Cities are rubble. Most buildings from before are a footprint and a scatter, if
that. This one is standing, roofed, with its walls square and its doors on their
hinges — and that is the strangest fact in the game.

Why it survived is not settled and does not need to be. It was nothing worth
aiming at, and the mountains stand between it and whatever came through. Take
your pick, or call it a miracle; travellers will.

There are others like it somewhere in the world. The odds of walking into one are
not worth counting on, and nobody plans a journey around the hope.

What this means for the writing:

- **Most travellers do not expect it.** They came for smoke, or for the road, or
  because staying put got worse. Finding walls is not what they were braced for,
  and a line can carry that without gawping at it.
- **A precious few have seen a whole building from before.** One, at a distance,
  years ago. That is a thing a person carries around and brings out, and it is a
  fair thing for a traveller to say.
- **A precious few have heard of this place.** Second or third hand, wrong in the
  details, from somebody who heard it from somebody. `bertram_stations` is one of
  these. The chain of stations that speaker walked is not from before — it was
  people keeping the road inside their own lifetime, mostly in dug-outs and
  lean-tos and whatever would hold a roof, and this valley's station happened to
  be the one place on it with walls. It failed the way everything else has.
  The before-time is not what is being grieved; the stations are.
- **One or two live near enough to hunt this valley** and have always steered
  wide of the building, on the grounds that something that stands when everything
  else fell is probably keeping something. A lit hearth in it is not reassuring.

## The Scribe

Down from the mountains: a couple of days' walk, three or four in bad going. Not
near, not far — the mountains make the difference, because a distance that would
be a morning on the flat is two days of picking a way down.

The Scribe did not know this place was here. Nobody sent them, nothing was
inherited, and there was no destination. They came down off the mountain, found
walls standing, and stayed — which is the same surprise every traveller has, and
the reason the Scribe is not above it.

## The four directions

**North — the Kiln, and the camp below it.** An old brickworks that a crew of
men made into a stronghold. They take a toll on the north road: goods first, then
time, then whatever else occurs to them. Twelve years ago they worked the old
road through this valley instead. There is nothing left to take here now, which
is most of why the valley is quiet. Below them is the north camp, a scavenger
settlement that rations hard and turns people out for arithmetic rather than
cruelty. It takes children in. It does not take whoever brought them.

**South — the Sill, and Sillford.** The river came up in one night and took the
village of Sillford with it. The ford is still crossable and is still where
people get separated from each other; two authored travellers have lost somebody
at that water. Being told you were lucky is a thing survivors of the flood have
heard too often to bear.

**West — the company.** Something that used to be an army out of the west and
still marches like one. It comes through settled ground, takes everything that
can walk, and moves on. It is the reason most people on this road are on it. It
is never seen and never described in detail; it is a direction people came from.

**East — the pass, and what is past it.** A settlement beyond the pass, poor
enough that fever herbs are worth a walk. Past that, rumour: grain, standing
water, terraces, a place that kept working. Everyone has been told; nobody has
been. The rumour is what most travellers are walking towards, and the game does
not say whether it is true.

## Who is on the road

Not adventurers. People with an errand, a debt, a body giving out, or somebody
missing. They are cautious in proportion to how bad the last few years were: a
stranger's fire is a decision, taken over days, and the first thing said is
usually about that decision rather than about the fire.

Nobody travels for its own sake. There are few enough people left that a road can
run empty for a week, and the road is where everything bad happens, so a person
stays where they are until staying is worse than going. Every traveller in the
court is somebody for whom staying got worse. That is the whole population of the
game.

Distances are therefore small and hard. A week's walk is a long way, a month is
an expedition somebody talks about for the rest of their life, and beyond that is
rumour. Nobody has been east past the pass. Nobody has seen the company that
comes out of the west, only its work. Two travellers who both came from the south
came from the same fifty miles of it.

Nobody arrives asking for religion, and nobody should. The book is the Scribe's
answer, not the traveller's request.

## Writing rules

These hold for `content/vignettes.ron` and `content/openings.ron` both. The ones
that can be checked are checked, in `crates/shared/src/lib.rs`.

- **Plain declaratives.** Short sentences, no exclamations, no archaism. The
  voice is tired and precise.
- **Arithmetic over adjectives.** Eleven weeks, forty-one names, four days
  sitting, three or four more days in the knees. Counting is how people in this
  world say how bad it is.
- **Violence implied, never shown.** What happened is over; what is on stage is
  somebody carrying it.
- **No self-description.** The face is generated. A traveller who says "as a
  woman" may be standing there with a beard. Talking about somebody else is
  fine — it is only the speaker who stays unspecified.
- **Nobody remembers the before.** No traveller has seen the world work, and
  neither did their grandparents. There is no "when I was young there were
  cities", no lost golden age anybody can testify to. Losses are recent and
  personal: a village, a crossing, a sister. The one thing from before that is
  fair to speak about is a *building* somebody saw still standing, because that
  is a thing seen with the eyes and not a memory.
- **Ages have to fit.** A profile in `visitors.rs` fixes whether the party is a
  grown walker, a pair of children, or an elder; a story about twelve years ago
  cannot be handed to a fourteen-year-old.
- **No greeting in a vignette.** That is the opening's job, and it is drawn
  separately.
- **Every story leaves an opening for the Scribe to be wrong.** Nobody states
  their need. They state the facts and the need is underneath, because the
  listening is the gameplay.

## If a model ever writes these

The authored pool is deliberately shaped so a generator could be dropped in
behind it rather than beside it — the game asks for a story that suits a party,
and does not care whether a person or a model wrote it.

What a prompt would need: this document; the writing rules above; the party's
shape (`Cast` — grown, elder, youth, child — and how many of them); the allowed
`need_id` list from `content/passages.ron`; and the instruction to return three
lines and at least two needs.

Two hazards worth knowing before starting:

- **Content controls.** This is a harsh setting, and a hosted model behind
  safety filtering may decline a line about a drowned village or a child alone.
  The server already treats a refusal as a failure and falls back to the
  reviewed local content (`crates/server/src/main.rs`), so the failure mode is
  degradation rather than a broken visit — but a generator that gets refused
  half the time is not a generator. Test the register before building on it.
- **The art has already been decided.** By the time anyone speaks, the traveller
  has been generated and is standing in the court. Generated words that
  contradict the visible person are worse than repeated words that do not.

The acceptance criteria for a generated line are exactly the tests that guard the
authored ones: three lines, real needs, nothing over-long, no greeting, no
self-description. A generator that passes those is shippable; one that does not
would have failed as hand-written content too.
