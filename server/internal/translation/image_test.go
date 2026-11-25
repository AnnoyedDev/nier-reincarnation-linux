package translation

import (
	"image"
	"image/color"
	"os"
	"path/filepath"
	"testing"

	"lunar-tear/server/mobile/unityasset"
)

// TestRebuildImageRoundTrip needs a game: LUNAR_GAME_DATA=<game>/data.
func TestRebuildImageRoundTrip(t *testing.T) {
	root := os.Getenv("LUNAR_GAME_DATA")
	if root == "" {
		t.Skip("LUNAR_GAME_DATA not set")
	}
	name := "ui)poem)en)season1)poem_main_1100101"
	raw, err := os.ReadFile(bannerBundlePath(filepath.Join(root, "assets", "revisions", "0"), name))
	if err != nil {
		t.Fatal(err)
	}
	img := image.NewNRGBA(image.Rect(0, 0, 392, 23))
	for y := 0; y < 23; y++ {
		for x := 0; x < 392; x++ {
			img.Set(x, y, color.NRGBA{uint8(x), uint8(y * 10), 7, uint8(255 - x/2)})
		}
	}
	out, _, err := RebuildImage(raw, name, img)
	if err != nil {
		t.Fatal(err)
	}
	// OpenBundle unmasks by the file's path under assetbundle/: put it there.
	dir := filepath.Join(t.TempDir(), "assetbundle", "ui", "poem", "en", "season1")
	os.MkdirAll(dir, 0o755)
	path := filepath.Join(dir, "poem_main_1100101.assetbundle")
	os.WriteFile(path, out, 0o644)
	b, err := unityasset.OpenBundle(path)
	if err != nil {
		t.Fatal(err)
	}
	tex, err := b.Textures()
	if err != nil || len(tex) != 1 {
		t.Fatalf("textures %v %d", err, len(tex))
	}
	for _, p := range [][2]int{{0, 0}, {391, 22}, {100, 5}} {
		i := (p[1]*392 + p[0]) * 4
		want := img.NRGBAAt(p[0], p[1])
		if got := tex[0].RGBA[i : i+4]; got[0] != want.R || got[1] != want.G || got[2] != want.B || got[3] != want.A {
			t.Fatalf("pixel %v: got %v want %v", p, got, want)
		}
	}
}
