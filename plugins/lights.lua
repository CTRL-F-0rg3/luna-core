luna.log("lights plugin loaded")

luna.command({ "lights on", "turn on the lights", "włącz światło" }, function(input)
	luna.speak("Włączam światła w salonie")
	-- luna.mqtt("home/living/lights", "ON")  -- podepniemy później
	return true
end)

luna.command({ "lights off", "wyłącz światło" }, function(input)
	luna.speak("Wyłączam światła")
	return true
end)
