//go:build !linux

package translation

import (
	"os"
	"path/filepath"
)

// syncFiles puts the given files of dir on disk, one by one.
func syncFiles(dir string, names []string) error {
	for _, n := range names {
		f, err := os.OpenFile(filepath.Join(dir, n), os.O_RDWR, 0)
		if err != nil {
			return err
		}
		err = f.Sync()
		f.Close()
		if err != nil {
			return err
		}
	}
	return nil
}
