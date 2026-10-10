# Copr's "make srpm" build method: https://docs.pagure.org/copr.copr/user_documentation.html#make-srpm
# Copr runs `make -f .copr/Makefile srpm outdir=<dir> spec=<spec>` in a checkout of this repo
# (with network access), then builds the source RPM offline.
outdir ?= $(CURDIR)/out
spec ?= packaging/fedora/oktospace.spec
top := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/..)

srpm:
	command -v cargo >/dev/null || dnf -y install cargo rust gcc
	command -v rpmbuild >/dev/null || dnf -y install rpm-build
	command -v git >/dev/null || dnf -y install git
	$(eval VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' $(top)/Cargo.toml | head -1))
	rm -rf $(top)/.copr/build && mkdir -p $(top)/.copr/build/oktospace-$(VERSION) $(outdir)
	cd $(top) && git ls-files -z | xargs -0 cp --parents -t .copr/build/oktospace-$(VERSION)
	cd $(top)/.copr/build/oktospace-$(VERSION) && mkdir -p .cargo && cargo vendor --locked vendor > .cargo/config.toml
	tar -C $(top)/.copr/build -czf $(outdir)/oktospace-$(VERSION)-vendored.tar.gz oktospace-$(VERSION)
	sed 's/^Version:.*/Version:        $(VERSION)/' $(top)/$(spec) > $(top)/.copr/build/oktospace.spec
	rpmbuild -bs --define "_sourcedir $(outdir)" --define "_srcrpmdir $(outdir)" $(top)/.copr/build/oktospace.spec
	rm -rf $(top)/.copr/build
