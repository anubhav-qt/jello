#include "listener.h"

bool is_valid_listens_to(float number) {
    return is_whole_within(number, listen_to_own, most_chains - 1);
}

Listener::Listener(int sample_rate) : share_(sample_rate * effect_crossfade_ms / 1000) {
    share_.jump_to(1.0);
}

void Listener::set(int listens_to) {
    if (listens_to == listens_to_) {
        return;
    }
    leaving_ = listens_to_;
    listens_to_ = listens_to;
    share_.jump_to(0.0);
}

void Listener::settle() {
    leaving_ = listens_to_;
    share_.jump_to(1.0);
}

void Listener::start(const float* own, const EffectContext& context) {
    own_ = own;
    chains_ = context.chains;
}

Stereo Listener::frame_of(const float* sound, int frame) {
    Stereo moment;
    moment.left = sound[2 * frame];
    moment.right = sound[2 * frame + 1];
    return moment;
}

const float* Listener::sound_of(int listens_to) const {
    if (listens_to == listen_to_own) {
        return own_;
    }
    return chains_[listens_to];
}

Stereo Listener::next(int frame) {
    double share = share_.next(1.0);
    Stereo heard = frame_of(sound_of(listens_to_), frame);
    if (share == 1.0) {
        return heard;
    }
    Stereo leaving = frame_of(sound_of(leaving_), frame);
    heard.left = leaving.left + (heard.left - leaving.left) * share;
    heard.right = leaving.right + (heard.right - leaving.right) * share;
    return heard;
}
