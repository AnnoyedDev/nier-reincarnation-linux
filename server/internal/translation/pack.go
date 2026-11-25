package translation

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// MaxPackBytes is the largest pack file read.
const MaxPackBytes = 64 << 20

// Pack is a language pack's game text. A pack comes from strangers: only these
// fields are read, and nothing in it names a file.
type Pack struct {
	Language, Name, Version string
	Hash                    string // hex sha256 of the file
	Game                    map[string]string
	Images                  map[string]string // translated image (PNG file) by bundle path, from images/ beside the pack
	Videos                  map[string]string // translated movie by resource name (mm…_en.mp4), from videos/ beside the pack
	Dropped                 int               // values left out: empty, not text, or with a line break
}

// LoadPack reads a pack file (format 1). Values that are empty (shown in
// English), not strings, or contain a line break (which would split the
// game's key:value line) are left out and counted.
func LoadPack(path string) (*Pack, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	data, err := io.ReadAll(io.LimitReader(f, MaxPackBytes+1))
	if err != nil {
		return nil, err
	}
	if len(data) > MaxPackBytes {
		return nil, fmt.Errorf("pack is larger than %d MB", MaxPackBytes>>20)
	}
	var raw struct {
		Format   int                        `json:"format"`
		Language string                     `json:"language"`
		Name     string                     `json:"name"`
		Version  string                     `json:"version"`
		Game     map[string]json.RawMessage `json:"game"`
	}
	if err := json.Unmarshal(data, &raw); err != nil {
		return nil, fmt.Errorf("pack is not valid: %w", err)
	}
	if raw.Format != 1 {
		return nil, fmt.Errorf("pack format %d is not supported", raw.Format)
	}
	if strings.TrimSpace(raw.Language) == "" {
		return nil, errors.New("pack has no language")
	}
	sum := sha256.Sum256(data)
	p := &Pack{Language: raw.Language, Name: raw.Name, Version: raw.Version, Game: map[string]string{}}
	p.Images, sum = packFiles(filepath.Join(filepath.Dir(path), "images"), ".png", sum)
	p.Videos, sum = packFiles(filepath.Join(filepath.Dir(path), "videos"), ".mp4", sum)
	p.Hash = hex.EncodeToString(sum[:])
	for key, value := range raw.Game {
		var s string
		if json.Unmarshal(value, &s) != nil || s == "" || strings.ContainsAny(s, "\r\n") {
			p.Dropped++
			continue
		}
		p.Game[key] = s
	}
	return p, nil
}

// packFiles lists the translated files of one kind: images as <dir>/<bundle
// path, ")" for "/">.png (keyed without .png), movies as <dir>/<resource name>
// (keyed with .mp4, as list.bin names them). Their names, sizes and times join
// the pack's hash (reading every file at each start would be slow), so
// changing one makes a new build.
func packFiles(dir, ext string, packSum [32]byte) (map[string]string, [32]byte) {
	images := map[string]string{}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return images, packSum
	}
	h := sha256.New()
	h.Write(packSum[:])
	names := []string{}
	for _, e := range entries {
		if n := e.Name(); !e.IsDir() && strings.HasSuffix(n, ext) {
			names = append(names, n)
		}
	}
	sort.Strings(names)
	for _, n := range names {
		info, err := os.Stat(filepath.Join(dir, n))
		if err != nil {
			continue
		}
		key := n
		if ext == ".png" {
			key = strings.TrimSuffix(n, ext)
		}
		images[key] = filepath.Join(dir, n)
		fmt.Fprintf(h, "%s:%d:%d\n", n, info.Size(), info.ModTime().UnixNano())
	}
	var sum [32]byte
	copy(sum[:], h.Sum(nil))
	return images, sum
}
