// Command tex-export writes every texture of the given bundles as PNG files:
// <out>/<bundle path with ")" for "/">__<texture name>.png, for translating
// the text drawn in images.
//
//	tex-export --data <game data folder> --out dir ui/help/en/foo ...
//	(bundle paths relative to the assetbundle folder, without .assetbundle;
//	 "-" reads them from stdin, one per line)
package main

import (
	"bufio"
	"flag"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"

	"lunar-tear/server/mobile/unityasset"
)

func main() {
	data := flag.String("data", "data", "the game data folder")
	out := flag.String("out", "textures", "output folder")
	flag.Parse()
	root := filepath.Join(*data, "assets", "revisions", "0", "assetbundle")
	paths := flag.Args()
	if len(paths) == 1 && paths[0] == "-" {
		paths = nil
		s := bufio.NewScanner(os.Stdin)
		for s.Scan() {
			if p := strings.TrimSpace(s.Text()); p != "" {
				paths = append(paths, p)
			}
		}
	}
	if err := os.MkdirAll(*out, 0o755); err != nil {
		log.Fatal(err)
	}
	for _, p := range paths {
		b, err := unityasset.OpenBundle(filepath.Join(root, p+".assetbundle"))
		if err != nil {
			log.Printf("%s: %v", p, err)
			continue
		}
		for name, file := range b.Files {
			if sf, err := unityasset.ParseSerializedFile(file); err == nil {
				objs, _ := sf.Objects(28) // Texture2D
				for _, o := range objs {
					fmt.Printf("#info %s %s fmt=%v mips=%v size=%v stream=%v\n", p, name, o.Fields["m_TextureFormat"], o.Fields["m_MipCount"], o.Fields["m_CompleteImageSize"], o.Fields["m_StreamData"])
				}
			}
		}
		textures, err := b.Textures()
		if err != nil {
			log.Printf("%s: %v", p, err)
			continue
		}
		for _, t := range textures {
			png, err := t.PNG()
			if err != nil {
				log.Printf("%s %s: %v", p, t.Name, err)
				continue
			}
			name := strings.ReplaceAll(p, "/", ")") + "__" + t.Name + ".png"
			if err := os.WriteFile(filepath.Join(*out, name), png, 0o644); err != nil {
				log.Fatal(err)
			}
			fmt.Printf("%s\t%s\t%dx%d\n", p, t.Name, t.Width, t.Height)
		}
	}
}
