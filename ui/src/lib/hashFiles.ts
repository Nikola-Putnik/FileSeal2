// Recognises a hash file chosen by the user: choosing one means "verify the
// files it lists" (cadrage, rule 10). Same names as the core accepts.

const HASH_FILE = /\.(md5|sha256)$|(^|[\\/])(md5|sha256)sums?(\.txt)?$/i;

export function isHashFile(path: string): boolean {
  return HASH_FILE.test(path);
}
