use rand::Rng;

const HAIKUS: [[&str; 3]; 10] = [
    [
        "Praise, never throw shade",
        "who helped this code become good?",
        "Name them. Remember.",
    ],
    [
        "Quality is not",
        "a gate you pass through once, but",
        "a garden you tend.",
    ],
    [
        "No one ships alone.",
        "lines were shaped by many hands",
        "you may never meet.",
    ],
    [
        "The careful review,",
        "the concern raised with kindness",
        "these are acts of love.",
    ],
    [
        "Trust accrues slowly,",
        "lonely comments, one by one",
        "there are no shortcuts.",
    ],
    [
        "What you measure, grows.",
        "So measure care, not just speed.",
        "Watch the garden bloom.",
    ],
    [
        "A blocker is not",
        "an obstacle, but a gift—",
        "someone chose to care.",
    ],
    [
        "We build on the work",
        "of those who built before us.",
        "Honor what you use.",
    ],
    [
        "Software remembers",
        "nothing of who made it good.",
        "So we write it down.",
    ],
    [
        "Ship with confidence",
        "not because nothing can break,",
        "but because you looked.",
    ],
];

pub fn run() {
    let idx = rand::rng().random_range(0..HAIKUS.len());
    let [a, b, c] = HAIKUS[idx];
    println!("\n  {a}\n  {b}\n  {c}\n");
}
