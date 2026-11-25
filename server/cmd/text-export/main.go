// Command text-export writes the game's English text (every "key:value" line of
// the text)en) bundles) as one JSON object, the starting point of a language
// pack (internal/translation, format 1).
//
//	text-export --data <game data folder> > en.json
package main

import (
	"encoding/json"
	"flag"
	"io/fs"
	"log"
	"os"
	"path/filepath"
	"strings"

	"lunar-tear/server/internal/translation"
)

func main() {
	data := flag.String("data", "data", "the game data folder (assets/revisions)")
	flag.Parse()
	root := filepath.Join(*data, "assets", "revisions", "0", "assetbundle")
	out := map[string]string{}
	conflicts := 0
	err := filepath.WalkDir(filepath.Join(root, "text", "en"), func(path string, d fs.DirEntry, err error) error {
		if err != nil || d.IsDir() || !strings.HasSuffix(path, ".assetbundle") {
			return err
		}
		rel, _ := filepath.Rel(root, strings.TrimSuffix(path, ".assetbundle"))
		name := strings.ReplaceAll(filepath.ToSlash(rel), "/", ")")
		raw, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		// The edit callback only reads: nothing is rebuilt.
		_, _, _, err = translation.RebuildText(raw, name, func(text string) (string, bool) {
			for _, line := range strings.Split(text, "\n") {
				body := strings.TrimSuffix(line, "\r")
				if s := strings.TrimSpace(body); s == "" || strings.HasPrefix(s, "//") {
					continue
				}
				head, value, ok := strings.Cut(body, ":")
				key := strings.TrimSpace(head)
				if !ok || key == "" {
					continue
				}
				if old, seen := out[key]; seen && old != value {
					conflicts++
				}
				out[key] = value
			}
			return text, false
		})
		if err != nil {
			log.Printf("%s: %v", name, err)
		}
		return nil
	})
	if err != nil {
		log.Fatal(err)
	}
	log.Printf("%d keys (%d keys with differing values across bundles)", len(out), conflicts)
	enc := json.NewEncoder(os.Stdout)
	enc.SetEscapeHTML(false)
	enc.SetIndent("", " ")
	if err := enc.Encode(out); err != nil {
		log.Fatal(err)
	}
}
