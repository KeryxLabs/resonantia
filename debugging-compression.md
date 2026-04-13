 Running `target/debug/resonantia`
sttp runtime ready: sttp-core-rs (surrealdb local: surrealkv:///home/theelevators/.config/io.keryxlabs.resonantia/sttp-local.db)
VM 0x7f0465406000 on pid 60721 received NeedDebuggerBreak trap
compose encode raw model output · session=dinner recipes chars=3793
-----BEGIN ENCODE RAW-----
⏣0{
    timestamp:   [Inferred Current Time, e.g., 2024-01-01T00:00:00Z],
    tier:        raw,
    session_id:  dinner recipes,
    user_avec:   { stability: 0.8, friction: 0.2, logic: 0.9, autonomy: 0.5, psi: 0.75},
    model_avec:  { stability: 0.9, friction: 0.15, logic: 0.95, autonomy: 0.6, psi: 0.8},
    schema_version: "1.0",
    user_avec: {
        stability: 0.8,
        friction: 0.2,
        logic: 0.9,
        autonomy: 0.5,
        psi: 0.75
    },
    model_avec: {
        stability: 0.9,
        friction: 0.15,
        logic: 0.95,
        autonomy: 0.6,
        psi: 0.8
    }
} ⟩
◈⟨
    ⏣0{
        timestamp:   [Inferred Current Time, e.g., 2024-01-01T00:00:00Z],
        tier:        raw,
        session_id:  dinner recipes,
        user_avec:   { stability: 0.8, friction: 0.2, logic: 0.9, autonomy: 0.5, psi: 0.75 },
        model_avec:  { stability: 0.9, friction: 0.15, logic: 0.95, autonomy: 0.6, psi: 0.8 },
        content: {
            turn: 1,
            speaker: "USER",
            text: "hello! mind helping me figure out what to make for dinner? I got some chicken, potatoes, frozen veggies, orzo, dairy stuff, rice, and sun dried tomatoes!!",
            context: "Ingredient list provided for dinner planning."
        },
        content: {
            turn: 2,
            speaker: "ASSISTANT",
            text: "That sounds like a fun challenge! You have some really versatile ingredients there.\n\nHere are a few ideas to get your culinary inspiration flowing:\n\n1. **Chicken & Orzo with Sun-Dried Tomatoes:** You could cook the chicken and toss it with the orzo, adding the sun-dried tomatoes for savory flavor, and maybe some dairy (like a little cheese or cream) to make it creamy.\n2. **Chicken and Rice Bowls:** A simple but satisfying option. Cook the chicken and rice, and toss in the frozen veggies and sun-dried tomatoes for a hearty meal.\n3. **Chicken & Potato Hash:** Dice up the potatoes and chicken, sauté them with the frozen veggies, and maybe a splash of dairy to make it richer.\n4. **Creamy Orzo:** If you have enough dairy, you could make a creamy chicken and sun-dried tomato orzo dish.\n\nWhich direction are you leaning—something pasta-based, rice-based, or a potato-heavy meal?",
            context: "Recipe suggestions based on available ingredients."
        },
        content: {
            turn: 3,
            speaker: "USER",
            text: "ooooo creazy orzo sounds so good",
            context: "User selects creamy orzo."
        },
        content: {
            turn: 4,
            speaker: "ASSISTANT",
            text: "That sounds like a wonderful choice! Creamy orzo is perfect when you have those rich ingredients.\n\nSince you have chicken, dairy, and sun-dried tomatoes, here are a few ways you could approach that creamy orzo:\n\n1. **Chicken Cream Sauce:** Cook the chicken, then create a simple sauce using the dairy (milk, cream, or even cheese if you have it) mixed with some of the sun-dried tomatoes and perhaps some seasonings. Toss this sauce with the cooked orzo and veggies.\n2. **Sun-Dried Tomato & Chicken Bake:** Layer the orzo, chicken, and veggies, and top with a creamy sauce made from the dairy and the tomatoes.\n3. **Potato & Orzo:** If you want to lean into the potato aspect, you could mix the potatoes into the orzo base for a heartier, creamier texture.\n\nWhat kind of dairy items do you have on hand (milk, cheese, heavy cream)? That will help narrow down the perfect creamy flavor!",
            context: "Specific creamy orzo recipes provided, prompting for dairy inventory."
        }
    }
⟩
⍉⟨
    rho: 0.85,
    kappa: 0.92,
    psi: 0.81,
    compression_avec: { stability: 0.91, friction: 0.18, logic: 0.96, autonomy: 0.62, psi: 0.83 }
} ⟩
-----END ENCODE RAW-----
compose encode normalized candidate · session=dinner recipes chars=3793
-----BEGIN ENCODE NORMALIZED-----
⏣0{
    timestamp:   [Inferred Current Time, e.g., 2024-01-01T00:00:00Z],
    tier:        raw,
    session_id:  dinner recipes,
    user_avec:   { stability: 0.8, friction: 0.2, logic: 0.9, autonomy: 0.5, psi: 0.75},
    model_avec:  { stability: 0.9, friction: 0.15, logic: 0.95, autonomy: 0.6, psi: 0.8},
    schema_version: "1.0",
    user_avec: {
        stability: 0.8,
        friction: 0.2,
        logic: 0.9,
        autonomy: 0.5,
        psi: 0.75
    },
    model_avec: {
        stability: 0.9,
        friction: 0.15,
        logic: 0.95,
        autonomy: 0.6,
        psi: 0.8
    }
} ⟩
◈⟨
    ⏣0{
        timestamp:   [Inferred Current Time, e.g., 2024-01-01T00:00:00Z],
        tier:        raw,
        session_id:  dinner recipes,
        user_avec:   { stability: 0.8, friction: 0.2, logic: 0.9, autonomy: 0.5, psi: 0.75 },
        model_avec:  { stability: 0.9, friction: 0.15, logic: 0.95, autonomy: 0.6, psi: 0.8 },
        content: {
            turn: 1,
            speaker: "USER",
            text: "hello! mind helping me figure out what to make for dinner? I got some chicken, potatoes, frozen veggies, orzo, dairy stuff, rice, and sun dried tomatoes!!",
            context: "Ingredient list provided for dinner planning."
        },
        content: {
            turn: 2,
            speaker: "ASSISTANT",
            text: "That sounds like a fun challenge! You have some really versatile ingredients there.\n\nHere are a few ideas to get your culinary inspiration flowing:\n\n1. **Chicken & Orzo with Sun-Dried Tomatoes:** You could cook the chicken and toss it with the orzo, adding the sun-dried tomatoes for savory flavor, and maybe some dairy (like a little cheese or cream) to make it creamy.\n2. **Chicken and Rice Bowls:** A simple but satisfying option. Cook the chicken and rice, and toss in the frozen veggies and sun-dried tomatoes for a hearty meal.\n3. **Chicken & Potato Hash:** Dice up the potatoes and chicken, sauté them with the frozen veggies, and maybe a splash of dairy to make it richer.\n4. **Creamy Orzo:** If you have enough dairy, you could make a creamy chicken and sun-dried tomato orzo dish.\n\nWhich direction are you leaning—something pasta-based, rice-based, or a potato-heavy meal?",
            context: "Recipe suggestions based on available ingredients."
        },
        content: {
            turn: 3,
            speaker: "USER",
            text: "ooooo creazy orzo sounds so good",
            context: "User selects creamy orzo."
        },
        content: {
            turn: 4,
            speaker: "ASSISTANT",
            text: "That sounds like a wonderful choice! Creamy orzo is perfect when you have those rich ingredients.\n\nSince you have chicken, dairy, and sun-dried tomatoes, here are a few ways you could approach that creamy orzo:\n\n1. **Chicken Cream Sauce:** Cook the chicken, then create a simple sauce using the dairy (milk, cream, or even cheese if you have it) mixed with some of the sun-dried tomatoes and perhaps some seasonings. Toss this sauce with the cooked orzo and veggies.\n2. **Sun-Dried Tomato & Chicken Bake:** Layer the orzo, chicken, and veggies, and top with a creamy sauce made from the dairy and the tomatoes.\n3. **Potato & Orzo:** If you want to lean into the potato aspect, you could mix the potatoes into the orzo base for a heartier, creamier texture.\n\nWhat kind of dairy items do you have on hand (milk, cheese, heavy cream)? That will help narrow down the perfect creamy flavor!",
            context: "Specific creamy orzo recipes provided, prompting for dairy inventory."
        }
    }
⟩
⍉⟨
    rho: 0.85,
    kappa: 0.92,
    psi: 0.81,
    compression_avec: { stability: 0.91, friction: 0.18, logic: 0.96, autonomy: 0.62, psi: 0.83 }
} ⟩
-----END ENCODE NORMALIZED-----
compose encode retry requested · session=dinner recipes parserHint=MissingLayer: Missing required layer: ⊕⟨ 
compose encode raw model output · session=dinner recipes chars=3438
-----BEGIN ENCODE RAW-----
⏣0{
    timestamp:   2024-01-01T00:00:00Z,
    tier:        raw,
    session_id:  dinner recipes,
    user_avec:   { stability: 0.8, friction: 0.2, logic: 0.9, autonomy: 0.5, psi: 0.75},
    model_avec:  { stability: 0.9, friction: 0.15, logic: 0.95, autonomy: 0.6, psi: 0.8}
} ⟩
◈⟨
    ⏣0{
        timestamp:   2024-01-01T00:00:00Z,
        tier:        raw,
        session_id:  dinner recipes,
        user_avec:   { stability: 0.8, friction: 0.2, logic: 0.9, autonomy: 0.5, psi: 0.75 },
        model_avec:  { stability: 0.9, friction: 0.15, logic: 0.95, autonomy: 0.6, psi: 0.8 },
        content: {
            turn: 1,
            speaker: "USER",
            text: "hello! mind helping me figure out what to make for dinner? I got some chicken, potatoes, frozen veggies, orzo, dairy stuff, rice, and sun dried tomatoes!!",
            context: "Ingredient list provided for dinner planning."
        },
        content: {
            turn: 2,
            speaker: "ASSISTANT",
            text: "That sounds like a fun challenge! You have some really versatile ingredients there.\n\nHere are a few ideas to get your culinary inspiration flowing:\n\n1. **Chicken & Orzo with Sun-Dried Tomatoes:** You could cook the chicken and toss it with the orzo, adding the sun-dried tomatoes for savory flavor, and maybe some dairy (like a little cheese or cream) to make it creamy.\n2. **Chicken and Rice Bowls:** A simple but satisfying option. Cook the chicken and rice, and toss in the frozen veggies and sun-dried tomatoes for a hearty meal.\n3. **Chicken & Potato Hash:** Dice up the potatoes and chicken, sauté them with the frozen veggies, and maybe a splash of dairy to make it richer.\n4. **Creamy Orzo:** If you have enough dairy, you could make a creamy chicken and sun-dried tomato orzo dish.\n\nWhich direction are you leaning—something pasta-based, rice-based, or a potato-heavy meal?",
            context: "Recipe suggestions based on available ingredients."
        },
        content: {
            turn: 3,
            speaker: "USER",
            text: "ooooo creazy orzo sounds so good",
            context: "User selects creamy orzo."
        },
        content: {
            turn: 4,
            speaker: "ASSISTANT",
            text: "That sounds like a wonderful choice! Creamy orzo is perfect when you have those rich ingredients.\n\nSince you have chicken, dairy, and sun-dried tomatoes, here are a few ways you could approach that creamy orzo:\n\n1. **Chicken Cream Sauce:** Cook the chicken, then create a simple sauce using the dairy (milk, cream, or even cheese if you have it) mixed with some of the sun-dried tomatoes and perhaps some seasonings. Toss this sauce with the cooked orzo and veggies.\n2. **Sun-Dried Tomato & Chicken Bake:** Layer the orzo, chicken, and veggies, and top with a creamy sauce made from the dairy and the tomatoes.\n3. **Potato & Orzo:** If you want to lean into the potato aspect, you could mix the potatoes into the orzo base for a heartier, creamier texture.\n\nWhat kind of dairy items do you have on hand (milk, cheese, heavy cream)? That will help narrow down the perfect creamy flavor!",
            context: "Specific creamy orzo recipes provided, prompting for dairy inventory."
        }
    }
⟩
⍉⟨
    rho: 0.85,
    kappa: 0.92,
    psi: 0.81,
    compression_avec: { stability: 0.91, friction: 0.18, logic: 0.96, autonomy: 0.62, psi: 0.83 }
} ⟩
-----END ENCODE RAW-----
compose encode normalized candidate · session=dinner recipes chars=3438
-----BEGIN ENCODE NORMALIZED-----
⏣0{
    timestamp:   2024-01-01T00:00:00Z,
    tier:        raw,
    session_id:  dinner recipes,
    user_avec:   { stability: 0.8, friction: 0.2, logic: 0.9, autonomy: 0.5, psi: 0.75},
    model_avec:  { stability: 0.9, friction: 0.15, logic: 0.95, autonomy: 0.6, psi: 0.8}
} ⟩
◈⟨
    ⏣0{
        timestamp:   2024-01-01T00:00:00Z,
        tier:        raw,
        session_id:  dinner recipes,
        user_avec:   { stability: 0.8, friction: 0.2, logic: 0.9, autonomy: 0.5, psi: 0.75 },
        model_avec:  { stability: 0.9, friction: 0.15, logic: 0.95, autonomy: 0.6, psi: 0.8 },
        content: {
            turn: 1,
            speaker: "USER",
            text: "hello! mind helping me figure out what to make for dinner? I got some chicken, potatoes, frozen veggies, orzo, dairy stuff, rice, and sun dried tomatoes!!",
            context: "Ingredient list provided for dinner planning."
        },
        content: {
            turn: 2,
            speaker: "ASSISTANT",
            text: "That sounds like a fun challenge! You have some really versatile ingredients there.\n\nHere are a few ideas to get your culinary inspiration flowing:\n\n1. **Chicken & Orzo with Sun-Dried Tomatoes:** You could cook the chicken and toss it with the orzo, adding the sun-dried tomatoes for savory flavor, and maybe some dairy (like a little cheese or cream) to make it creamy.\n2. **Chicken and Rice Bowls:** A simple but satisfying option. Cook the chicken and rice, and toss in the frozen veggies and sun-dried tomatoes for a hearty meal.\n3. **Chicken & Potato Hash:** Dice up the potatoes and chicken, sauté them with the frozen veggies, and maybe a splash of dairy to make it richer.\n4. **Creamy Orzo:** If you have enough dairy, you could make a creamy chicken and sun-dried tomato orzo dish.\n\nWhich direction are you leaning—something pasta-based, rice-based, or a potato-heavy meal?",
            context: "Recipe suggestions based on available ingredients."
        },
        content: {
            turn: 3,
            speaker: "USER",
            text: "ooooo creazy orzo sounds so good",
            context: "User selects creamy orzo."
        },
        content: {
            turn: 4,
            speaker: "ASSISTANT",
            text: "That sounds like a wonderful choice! Creamy orzo is perfect when you have those rich ingredients.\n\nSince you have chicken, dairy, and sun-dried tomatoes, here are a few ways you could approach that creamy orzo:\n\n1. **Chicken Cream Sauce:** Cook the chicken, then create a simple sauce using the dairy (milk, cream, or even cheese if you have it) mixed with some of the sun-dried tomatoes and perhaps some seasonings. Toss this sauce with the cooked orzo and veggies.\n2. **Sun-Dried Tomato & Chicken Bake:** Layer the orzo, chicken, and veggies, and top with a creamy sauce made from the dairy and the tomatoes.\n3. **Potato & Orzo:** If you want to lean into the potato aspect, you could mix the potatoes into the orzo base for a heartier, creamier texture.\n\nWhat kind of dairy items do you have on hand (milk, cheese, heavy cream)? That will help narrow down the perfect creamy flavor!",
            context: "Specific creamy orzo recipes provided, prompting for dairy inventory."
        }
    }
⟩
⍉⟨
    rho: 0.85,
    kappa: 0.92,
    psi: 0.81,
    compression_avec: { stability: 0.91, friction: 0.18, logic: 0.96, autonomy: 0.62, psi: 0.83 }
} ⟩
-----END ENCODE NORMALIZED-----
