class LittleBoxes < Formula
  desc "Adds boxes around stdin. Optionally adds a title"
  homepage "https://github.com/giodamelio/little_boxes"
  url "https://github.com/giodamelio/little_boxes/archive/1.6.0.tar.gz"
  sha256 "c686d3e347b4a2fdf8dfa3853bf6ca5e0b2f8d20c69fd24a234547e3cb002260"
  head "https://github.com/giodamelio/little_boxes.git"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--root", prefix,
                               "--path", "."
    man1.install "little_boxes.1"
  end

  test do
    system "#{bin}/little_boxes", "-V"
  end
end
