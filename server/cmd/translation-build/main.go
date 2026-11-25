// Command translation-build builds the active language pack (texts and
// images) for the game's list right away, so the server finds it finished
// at its next start instead of building it while the game runs.
//
//	translation-build --data <game data folder> [--force]
package main

import (
	"flag"
	"log"
	"time"

	"lunar-tear/server/internal/translation"
)

func main() {
	data := flag.String("data", "data", "the game data folder (holds assets/ and translation/)")
	platform := flag.String("platform", "android", "android or ios")
	force := flag.Bool("force", false, "rebuild even if a finished build exists (after changing images)")
	flag.Parse()
	start := time.Now()
	m, err := translation.Prebuild(*data, *platform, *force)
	if err != nil {
		log.Fatal(err)
	}
	log.Printf("build %s: %d bundles, %d kept in English, %v", m.ID, len(m.Entries), len(m.Skipped), time.Since(start).Round(time.Second))
}
