#pragma once

// A value that never jumps, so changing it never clicks: it moves toward the
// value asked for in a straight line, at most `step` each sample. Only the
// audio thread uses it.
class Ease {
public:
    Ease(float start, float step) : wanted_(start), heard_(start), step_(step) {}

    // The value to move to.
    void set(float wanted) {
        wanted_ = wanted;
    }

    // Moves one sample on and returns the value heard there.
    float next() {
        if (heard_ < wanted_) {
            heard_ += step_;
            if (heard_ > wanted_) {
                heard_ = wanted_;
            }
        } else if (heard_ > wanted_) {
            heard_ -= step_;
            if (heard_ < wanted_) {
                heard_ = wanted_;
            }
        }
        return heard_;
    }

    // The value heard now.
    float heard() const {
        return heard_;
    }

    // True while it is still on its way.
    bool is_moving() const {
        return heard_ != wanted_;
    }

private:
    float wanted_;
    float heard_;
    float step_;
};
