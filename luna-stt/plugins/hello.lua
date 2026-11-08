luna.log("hello plugin loaded")

luna.command({"hello", "hi", "cześć", "siema"}, function(input)
    luna.speak("Cześć! Słucham cię, Kacper.")
    return true
end)
