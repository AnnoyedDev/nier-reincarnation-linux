package translation

import (
	"os"

	"golang.org/x/sys/unix"
)

// syncFiles puts the given files of dir on disk: one syncfs for the whole
// filesystem instead of an fsync per file (each one costs up to a second on a
// busy hard disk, and a build writes thousands).
func syncFiles(dir string, _ []string) error {
	d, err := os.Open(dir)
	if err != nil {
		return err
	}
	defer d.Close()
	return unix.Syncfs(int(d.Fd()))
}
