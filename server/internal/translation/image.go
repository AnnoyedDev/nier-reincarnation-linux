package translation

import (
	"bytes"
	"errors"
	"fmt"
	"hash/crc32"
	"image"
	"image/draw"
	"image/png"
	"io"
	"os"

	"lunar-tear/server/mobile/unityasset"
)

const (
	texture2D     = 28
	formatRGBA32  = 4
	maxImageBytes = 16 << 20 // a translated image's PNG
)

// RebuildImage gives a masked bundle's one texture new pixels: img, of the
// texture's size, stored uncompressed (RGBA32) whatever the original format,
// so no encoder is needed. The texture's format, size and stream fields are
// patched in place (fixed-size fields: the serialized file keeps its layout).
// crc is list.bin's, as buildBanner's.
func RebuildImage(raw []byte, name string, img image.Image) (out []byte, crc uint32, err error) {
	plain, err := unityasset.Unmask(raw, name)
	if err != nil {
		return nil, 0, err
	}
	b, err := readNodes(plain)
	if err != nil {
		return nil, 0, err
	}
	serialized, resS, err := bannerFiles(b)
	if err != nil {
		return nil, 0, err
	}
	sf, err := unityasset.ParseSerializedFile(serialized.data)
	if err != nil {
		return nil, 0, err
	}
	all, err := sf.FieldOffsets(texture2D)
	if err != nil {
		return nil, 0, err
	}
	if len(all) != 1 {
		return nil, 0, fmt.Errorf("bundle has %d textures, want 1", len(all))
	}
	f := all[0]
	data := bytes.Clone(sf.Data())
	order := sf.Order()
	for _, k := range []string{"m_Width", "m_Height", "m_CompleteImageSize", "m_TextureFormat", "m_MipCount", "m_StreamData.offset", "m_StreamData.size", "image data"} {
		if _, ok := f[k]; !ok {
			return nil, 0, fmt.Errorf("texture has no %s field", k)
		}
	}
	u32 := func(k string) int { return int(order.Uint32(data[f[k]:])) }
	w, h := u32("m_Width"), u32("m_Height")
	if img.Bounds().Dx() != w || img.Bounds().Dy() != h {
		return nil, 0, fmt.Errorf("image is %dx%d, the texture %dx%d", img.Bounds().Dx(), img.Bounds().Dy(), w, h)
	}
	if u32("image data") != 0 || u32("m_StreamData.offset") != 0 || u32("m_StreamData.size") != len(resS.data) {
		return nil, 0, errors.New("texture does not fill its .resS alone")
	}
	rgba := image.NewNRGBA(image.Rect(0, 0, w, h))
	draw.Draw(rgba, rgba.Rect, img, img.Bounds().Min, draw.Src)
	pixels := make([]byte, 0, w*h*4)
	for y := h - 1; y >= 0; y-- { // Unity stores the bottom row first
		pixels = append(pixels, rgba.Pix[y*rgba.Stride:y*rgba.Stride+w*4]...)
	}
	put := func(k string, v int) { order.PutUint32(data[f[k]:], uint32(v)) }
	put("m_TextureFormat", formatRGBA32)
	put("m_CompleteImageSize", len(pixels))
	put("m_MipCount", 1)
	put("m_StreamData.size", len(pixels))
	serialized.data, resS.data = data, pixels
	bundle, err := writeNodes(b)
	if err != nil {
		return nil, 0, err
	}
	marker := raw[0]
	if marker != 0x31 && marker != 0x32 {
		marker = 0x32
	}
	return Mask(bundle, name, marker), crc32.ChecksumIEEE(append(bytes.Clone(serialized.data), resS.data...)), nil
}

// readImage reads a translated image (a PNG of at most maxImageBytes).
func readImage(path string) (image.Image, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	data, err := io.ReadAll(io.LimitReader(f, maxImageBytes+1))
	if err != nil {
		return nil, err
	}
	if len(data) > maxImageBytes {
		return nil, fmt.Errorf("%s is larger than %d MB", path, maxImageBytes>>20)
	}
	return png.Decode(bytes.NewReader(data))
}
